use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use nalgebra::base::VecStorage;
use nalgebra::dimension::{Dyn, U1};
use nalgebra::linalg::SVD;
use nalgebra::{DMatrix, DVector, Matrix3, Matrix3x4, Vector2, Vector3, Vector4};

// Helper function to project a 3D point onto a 2D image plane.
fn project_point(P: Matrix3x4<f64>, x: Vector3<f64>) -> Vector2<f64> {
    let x_h = Vector4::new(x[0], x[1], x[2], 1.0);
    let x_proj = P * x_h;
    Vector2::new(x_proj[0] / x_proj[2], x_proj[1] / x_proj[2])
}

// This function does initial estimate of the 3D position using the DLT algorithm.
fn dlt_triangulation(P_list: &[Matrix3x4<f64>], pixels: &[Vector2<f64>]) -> Vector3<f64> {
    let n = P_list.len();
    let mut A = DMatrix::<f64>::zeros(2 * n, 4);
    for i in 0..n {
        let P = P_list[i];
        let pixel = pixels[i];
        let u = pixel[0];
        let v = pixel[1];
        for j in 0..4 {
            A[(2 * i, j)] = u * P[(2, j)] - P[(0, j)];
            A[(2 * i + 1, j)] = v * P[(2, j)] - P[(1, j)];
        }
    }
    let svd = SVD::new(A, false, true);
    let v_t = svd.v_t.expect("SVD with compute_v=true");
    let v = v_t.transpose();
    let X_h = v.column(v.ncols() - 1);
    Vector3::new(X_h[0] / X_h[3], X_h[1] / X_h[3], X_h[2] / X_h[3])
}

//Setup the problem for the trajectory of a projectile. 
//This includes the number of position parameters, the drag coefficient, the number of residuals, and the numerical Jacobian.
struct TrajectoryProblem {
    params: DVector<f64>,
    P_list: Vec<Matrix3x4<f64>>,
    pixels: Vec<Vec<Vector2<f64>>>,
    n_timesteps: usize,
    omega_phys: f64,
    dt: f64,
    g: Vector3<f64>,
    pixel_sigma: f64,
    physics_sigma: f64,
}

impl TrajectoryProblem {
    fn n_position(&self) -> usize {
        3 * self.n_timesteps
    }

    fn drag(&self) -> f64 {
        self.params[self.n_position()]
    }

    fn n_residuals(&self) -> usize {
        let n_cams = self.P_list.len();
        self.n_timesteps * n_cams * 2 + self.n_timesteps.saturating_sub(2) * 3
    }

    fn numerical_jacobian(&self, epsilon: f64) -> DMatrix<f64> {
        let n = self.params.len();
        let m = self.n_residuals();
        let _r0 = self.residuals().unwrap_or_else(|| DVector::zeros(m));
        let mut J = DMatrix::zeros(m, n);
        let mut params_plus = self.params.clone();
        let mut params_minus = self.params.clone();
        for j in 0..n {
            let xj = self.params[j];
            params_plus[j] = xj + epsilon;
            params_minus[j] = xj - epsilon;
            let prob_plus = TrajectoryProblem {
                params: params_plus.clone(),
                P_list: self.P_list.clone(),
                pixels: self.pixels.clone(),
                n_timesteps: self.n_timesteps,
                omega_phys: self.omega_phys,
                dt: self.dt,
                g: self.g,
                pixel_sigma: self.pixel_sigma,
                physics_sigma: self.physics_sigma,
            };
            let prob_minus = TrajectoryProblem {
                params: params_minus.clone(),
                P_list: self.P_list.clone(),
                pixels: self.pixels.clone(),
                n_timesteps: self.n_timesteps,
                omega_phys: self.omega_phys,
                dt: self.dt,
                g: self.g,
                pixel_sigma: self.pixel_sigma,
                physics_sigma: self.physics_sigma,
            };
            if let (Some(r_plus), Some(r_minus)) = (prob_plus.residuals(), prob_minus.residuals()) {
                for i in 0..m {
                    J[(i, j)] = (r_plus[i] - r_minus[i]) / (2.0 * epsilon)
                }
            }
            params_plus[j] = xj;
            params_minus[j] = xj;
        }
        J
    }

    fn covariance_from_jacobian(&self, jacobian: &DMatrix<f64>) -> Option<DMatrix<f64>> {
        let r = self.residuals()?;
        let m = r.len();
        let n = self.params.len();
        let jtj = jacobian.transpose() * jacobian;
        let jtj_inv = jtj.try_inverse()?;
        let dof = (m - n).max(1) as f64;
        let sigma2 = r.norm_squared() / dof;
        Some(jtj_inv * sigma2)
    }
}

// Function sets up and returns the trajectory of a projectile using the Levenberg-Marquardt algorithm.
impl LeastSquaresProblem<f64, Dyn, Dyn> for TrajectoryProblem {
    type ParameterStorage = VecStorage<f64, Dyn, U1>;
    type ResidualStorage = VecStorage<f64, Dyn, U1>;
    type JacobianStorage = VecStorage<f64, Dyn, Dyn>;

    fn set_params(&mut self, x: &DVector<f64>) {
        self.params.copy_from(x);
    }

    fn params(&self) -> DVector<f64> {
        self.params.clone()
    }

    fn residuals(&self) -> Option<DVector<f64>> {
        let n_cams = self.P_list.len();
        let mut residuals = Vec::with_capacity(self.n_timesteps * n_cams * 2 + self.n_timesteps.saturating_sub(2) * 3);
        for t in 0..self.n_timesteps {
            let X_t = Vector3::new(self.params[t * 3], self.params[t * 3 + 1], self.params[t * 3 + 2]);
            for c in 0..n_cams {
                let pred = project_point(self.P_list[c], X_t);
                let pix = self.pixels.get(c).and_then(|row| row.get(t)).copied().unwrap_or(Vector2::zeros());
                let r = (pred - pix) / self.pixel_sigma;
                residuals.push(r[0]);
                residuals.push(r[1]);
            }
        }
        // Discrete physics residual (matches Python `trajectory_residual`):
        // X_next - 2*X_curr + X_prev - g*dt^2 + drag * 0.5 * dt * (X_next - X_prev), scaled by physics_sigma.
        // The drag term is linear-in-velocity style using the centered finite-difference (X_next - X_prev).
        let drag_k = self.drag();
        for t in 1..self.n_timesteps.saturating_sub(1) {
            let X_prev = Vector3::new(self.params[(t - 1) * 3], self.params[(t - 1) * 3 + 1], self.params[(t - 1) * 3 + 2]);
            let X_curr = Vector3::new(self.params[t * 3], self.params[t * 3 + 1], self.params[t * 3 + 2]);
            let X_next = Vector3::new(self.params[(t + 1) * 3], self.params[(t + 1) * 3 + 1], self.params[(t + 1) * 3 + 2]);
            let phys_res = (X_next - 2.0 * X_curr + X_prev - self.g * self.dt * self.dt
                + drag_k * 0.5 * self.dt * (X_next - X_prev))
                / self.physics_sigma;
            residuals.push(self.omega_phys * phys_res[0]);
            residuals.push(self.omega_phys * phys_res[1]);
            residuals.push(self.omega_phys * phys_res[2]);
        }
        Some(DVector::from_vec(residuals))
    }
    
    //Use built in jacobian function from the LevenbergMarquardt library.
    fn jacobian(&self) -> Option<DMatrix<f64>> {
        None
    }
}

// This function optimizes the trajectory of a projectile using the Levenberg-Marquardt algorithm.
// Initial guess for the trajectory is gotten from DLT triangulation.
// Then to further optimize the trajectory, we use the Levenberg-Marquardt algorithm.
// We return the optimized trajectory and the covariance matrix.
// Args:
//     P_list: List of camera projection matrices. Should be gotten from camera calibration.
//     pixels: List of pixel coordinates for each camera at each timestep. Should be gotten from CV.
//     dt: Time step (1/fps).
//     g: Gravity vector.
//     drag: Drag coefficient.
//     pixel_sigma: Standard deviation of the pixel noise.
//     physics_sigma: Standard deviation of the physics noise.
//     omega_phys: Hyperparameter for the physics noise.
// Returns: trajectory, position covariance, optimized drag, LM success flag.
pub async fn optimize_trajectory(P_list: &[Matrix3x4<f64>], pixels: &[Vec<Vector2<f64>>], dt: f64, g: Option<Vector3<f64>>, drag: f64, pixel_sigma: f64, physics_sigma: f64, omega_phys: f64,) -> (Vec<Vector3<f64>>, DMatrix<f64>, f64, bool) {
    let g = g.unwrap_or_else(|| Vector3::new(0.0, 0.0, -9.81));
    let n_timesteps = pixels[0].len();
    let mut X_init = Vec::with_capacity(n_timesteps);
    for t in 0..n_timesteps {
        let mut pixel_t = Vec::new();
        for p in pixels {
            if let Some(v) = p.get(t) {
                pixel_t.push(*v);
            }
        }
        X_init.push(dlt_triangulation(P_list, &pixel_t));
    }
    let mut params_init: Vec<f64> = X_init.iter().flat_map(|v| vec![v[0], v[1], v[2]]).collect();
    params_init.push(drag);
    let problem = TrajectoryProblem {
        params: DVector::from_vec(params_init),
        P_list: P_list.to_vec(),
        pixels: pixels.to_vec(),
        n_timesteps,
        omega_phys,
        dt,
        g,
        pixel_sigma,
        physics_sigma,
    };
    let (problem, report) = LevenbergMarquardt::default().with_patience(800).minimize(problem);
    let success = report.termination.was_successful();
    let drag_opt = problem.drag();
    let X_opt: Vec<Vector3<f64>> = (0..n_timesteps)
        .map(|t| Vector3::new(problem.params[t * 3], problem.params[t * 3 + 1], problem.params[t * 3 + 2]))
        .collect();
    let jacobian = problem.numerical_jacobian(1e-7);
    let cov_full = problem.covariance_from_jacobian(&jacobian).unwrap_or_else(|| DMatrix::zeros(3 * n_timesteps + 1, 3 * n_timesteps + 1));
    let cov = cov_full.view((0, 0), (3 * n_timesteps, 3 * n_timesteps)).into_owned();
    (X_opt, cov, drag_opt, success)
}



//Everythng below here is for testing purposes
fn trajectory_residual(params: &DVector<f64>, p_list: &[Matrix3x4<f64>], pixels: &[Vec<Vector2<f64>>], n_timesteps: usize, omega_phys: f64, dt: f64, g: Vector3<f64>, pixel_sigma: f64, physics_sigma: f64) -> DVector<f64> {
    TrajectoryProblem {params: params.clone(),P_list: p_list.to_vec(),pixels: pixels.to_vec(),n_timesteps,omega_phys,dt,g,pixel_sigma,physics_sigma}.residuals().expect("residuals")    
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use rand_distr::{Distribution, Normal};

    fn assert_vec2_close(a: Vector2<f64>, exp: [f64; 2], atol: f64) {
        assert!((a[0] - exp[0]).abs() <= atol && (a[1] - exp[1]).abs() <= atol, "{a:?} vs {exp:?}");
    }

    fn assert_vec3_close(a: Vector3<f64>, b: Vector3<f64>, atol: f64) {
        assert!((a - b).norm() < atol, "{a:?} vs {b:?}");
    }

    fn assert_allclose_f64(a: f64, b: f64, rtol: f64, atol: f64) {
        assert!((a - b).abs() <= atol + rtol * b.abs(), "expected {b}, got {a} (rtol={rtol}, atol={atol})");
    }

    fn two_cameras() -> Vec<Matrix3x4<f64>> {
        vec![
            Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
            Matrix3x4::new(1.0, 0.0, 0.0, -0.55, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        ]
    }

    fn camera_look_at(cam_position: Vector3<f64>, target: Vector3<f64>) -> (Matrix3<f64>, Vector3<f64>) {
        let up = Vector3::new(0.0, 0.0, 1.0);
        let z = (target - cam_position).normalize();
        let mut x = up.cross(&z);
        if x.norm() < 1e-10 {
            x = Vector3::new(1.0, 0.0, 0.0);
        }
        x.normalize_mut();
        let y = z.cross(&x);
        let r = Matrix3::new(x[0], y[0], z[0], x[1], y[1], z[1], x[2], y[2], z[2]);
        let t = -r * cam_position;
        (r, t)
    }

    struct Scene {
        p_list: Vec<Matrix3x4<f64>>,
        pixels: Vec<Vec<Vector2<f64>>>,
        traj_true: Vec<Vector3<f64>>,
        g: Vector3<f64>,
        dt: f64,
        drag_coef: f64,
        n_timesteps: usize,
        n_cameras: usize,
    }

    fn main_style_scene(rng: &mut StdRng, n_cameras: usize, n_steps_cap: usize, dt: f64, g: Vector3<f64>, drag_coef: f64, pixel_noise_sigma: f64) -> Scene {
        let k_list: Vec<Matrix3<f64>> = (0..n_cameras)
            .map(|_| Matrix3::new(800.0, 0.0, 320.0, 0.0, 800.0, 240.0, 0.0, 0.0, 1.0))
            .collect();
        let center = Vector3::zeros();
        let radius = 4.0_f64;
        let height = 2.0_f64;
        let angles: Vec<f64> = if n_cameras == 3 {
            vec![0.0, 2.0 * std::f64::consts::PI / 3.0, 4.0 * std::f64::consts::PI / 3.0]
        } else {
            (0..n_cameras).map(|i| 2.0 * std::f64::consts::PI * i as f64 / n_cameras as f64).collect()
        };
        let mut p_list = Vec::with_capacity(n_cameras);
        for i in 0..n_cameras {
            let cam_pos = Vector3::new(radius * angles[i].cos(), radius * angles[i].sin(), height);
            let (r, t) = camera_look_at(cam_pos, center);
            let rt = Matrix3x4::from_fn(|row, col| if col < 3 { r[(row, col)] } else { t[row] });
            p_list.push(k_list[i] * rt);
        }

        let x0 = Vector3::new(0.0, 0.0, 1.5);
        let v0 = Vector3::new(1.0, 1.0, 2.5);
        let z_floor = 0.2_f64;
        let mut traj_true = Vec::new();
        let mut x = x0;
        let mut v = v0;
        for _ in 0..n_steps_cap {
            traj_true.push(x);
            let acc = g - drag_coef * v;
            v = v + acc * dt;
            x = x + v * dt;
            if x[2] < z_floor {
                break;
            }
        }
        let n_timesteps = traj_true.len();
        let normal = Normal::new(0.0, pixel_noise_sigma).unwrap();
        let mut pixels: Vec<Vec<Vector2<f64>>> = Vec::new();
        for i in 0..n_cameras {
            let mut row = Vec::with_capacity(n_timesteps);
            for t in 0..n_timesteps {
                let p = project_point(p_list[i], traj_true[t]);
                row.push(p + Vector2::new(normal.sample(rng), normal.sample(rng)));
            }
            pixels.push(row);
        }

        Scene {
            p_list,
            pixels,
            traj_true,
            g,
            dt,
            drag_coef,
            n_timesteps,
            n_cameras,
        }
    }

    fn drag_calc(x_prev: Vector3<f64>, x_curr: Vector3<f64>, g: Vector3<f64>, drag: f64, dt: f64) -> Vector3<f64> {
        let denom = 1.0 + drag * dt / 2.0;
        (2.0 * x_curr - x_prev + g * dt * dt + drag * dt / 2.0 * x_prev) / denom
    }

    #[test]
    fn test_simple_projection() {
        let p = Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let uv = project_point(p, Vector3::new(2.0, 4.0, 2.0));
        assert_vec2_close(uv, [1.0, 2.0], 1e-12);
    }

    #[test]
    fn test_origin_on_optical_axis() {
        let p = Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let uv = project_point(p, Vector3::new(0.0, 0.0, 1.0));
        assert_vec2_close(uv, [0.0, 0.0], 1e-12);
    }

    #[test]
    fn test_with_intrinsics() {
        let fx = 800.0_f64;
        let fy = 800.0_f64;
        let cx = 320.0_f64;
        let cy = 240.0_f64;
        let k = Matrix3::new(fx, 0.0, cx, 0.0, fy, cy, 0.0, 0.0, 1.0);
        let r = Matrix3::identity();
        let t = Vector3::zeros();
        let rt = Matrix3x4::from_fn(|row, col| if col < 3 { r[(row, col)] } else { t[row] });
        let p = k * rt;
        let uv = project_point(p, Vector3::new(1.0, 0.5, 5.0));
        let expected = Vector2::new(fx * (1.0 / 5.0) + cx, fy * (0.5 / 5.0) + cy);
        assert!((uv - expected).norm() < 1e-9);
    }

    #[test]
    fn test_recover_point_two_cameras() {
        let p0 = Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let p1 = Matrix3x4::new(1.0, 0.0, 0.0, -0.55, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let x_true = Vector3::new(0.3, -0.2, 2.5);
        let u0 = project_point(p0, x_true);
        let u1 = project_point(p1, x_true);
        let x_hat = dlt_triangulation(&[p0, p1], &[u0, u1]);
        assert_vec3_close(x_hat, x_true, 1e-5);
    }

    #[test]
    fn test_recover_point_three_cameras() {
        let mut rng = StdRng::seed_from_u64(0);
        let normal = Normal::new(0.0, 1.0).unwrap();
        let mut p_list = Vec::new();
        for _ in 0..3 {
            let a = Matrix3::from_fn(|_, _| normal.sample(&mut rng));
            let svd = SVD::new(a, true, true);
            let mut q = svd.u.expect("u");
            if q.determinant() < 0.0 {
                q.set_column(0, &-q.column(0));
            }
            let tt = Vector3::new(normal.sample(&mut rng), normal.sample(&mut rng), normal.sample(&mut rng)) * 0.3;
            let kk = Matrix3::new(600.0, 0.0, 320.0, 0.0, 600.0, 240.0, 0.0, 0.0, 1.0);
            let rt = Matrix3x4::from_fn(|row, col| if col < 3 { q[(row, col)] } else { tt[row] });
            p_list.push(kk * rt);
        }
        let x_true = Vector3::new(0.1, 0.15, 3.0);
        let pix: Vec<Vector2<f64>> = p_list.iter().map(|p| project_point(*p, x_true)).collect();
        let x_hat = dlt_triangulation(&p_list, &pix);
        assert_vec3_close(x_hat, x_true, 1e-4);
    }

    #[test]
    fn test_zero_variance() {
        let n_timesteps = 3;
        let n_cams = 2;
        let dt = 0.1;
        let g = Vector3::new(0.0, 0.0, -10.0);
        let drag = 0.25;
        let p_list = two_cameras();
        let base = Vector3::new(0.1, -0.05, 14.0);
        let x2 = drag_calc(base, base, g, drag, dt);
        let mut params_vec: Vec<f64> = Vec::new();
        for v in [base, base, x2] {
            params_vec.extend_from_slice(&[v[0], v[1], v[2]]);
        }
        params_vec.push(drag);
        let params = DVector::from_vec(params_vec);
        let mut pixels: Vec<Vec<Vector2<f64>>> = Vec::new();
        for i in 0..n_cams {
            let row: Vec<Vector2<f64>> = (0..n_timesteps)
                .map(|t| {
                    let xt = Vector3::new(params[t * 3], params[t * 3 + 1], params[t * 3 + 2]);
                    project_point(p_list[i], xt)
                })
                .collect();
            pixels.push(row);
        }
        let res = trajectory_residual(&params, &p_list, &pixels, n_timesteps, 1.0, dt, g, 1.0, 1.0);
        for i in 0..res.len() {
            assert!(res[i].abs() < 1e-9, "res[{i}] = {}", res[i]);
        }
    }

    #[test]
    fn test_wrong_drag() {
        let n_timesteps = 3;
        let dt = 0.1;
        let g = Vector3::new(0.0, 0.0, -9.81);
        let drag_true = 0.1;
        let p0 = Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0);
        let p_list = vec![p0];
        let base = Vector3::new(0.0, 0.0, 8.0);
        let x2 = drag_calc(base, base, g, drag_true, dt);
        let mut params_vec: Vec<f64> = Vec::new();
        for v in [base, base, x2] {
            params_vec.extend_from_slice(&[v[0], v[1], v[2]]);
        }
        params_vec.push(99.0);
        let params = DVector::from_vec(params_vec);
        let pixels: Vec<Vec<Vector2<f64>>> = vec![(0..n_timesteps)
            .map(|t| {
                let xt = Vector3::new(params[t * 3], params[t * 3 + 1], params[t * 3 + 2]);
                project_point(p_list[0], xt)
            })
            .collect()];
        let res = trajectory_residual(&params, &p_list, &pixels, n_timesteps, 1.0, dt, g, 1.0, 1.0);
        assert!(res.norm() > 0.05);
    }

    #[test]
    fn test_output_shape() {
        let n_timesteps = 4usize;
        let n_cams = 2usize;
        let t = n_timesteps;
        let c = n_cams;
        let n_res = t * c * 2 + t.saturating_sub(2) * 3;
        let xyz = [0.2_f64, -0.1, 6.0];
        let mut params_vec: Vec<f64> = (0..t).flat_map(|_| xyz.iter().copied()).collect();
        params_vec.push(0.0);
        let params = DVector::from_vec(params_vec);
        let p_list = two_cameras();
        let mut pixels: Vec<Vec<Vector2<f64>>> = Vec::new();
        for i in 0..c {
            let row: Vec<Vector2<f64>> = (0..t).map(|tt| {let x = Vector3::new(params[tt * 3], params[tt * 3 + 1], params[tt * 3 + 2]); project_point(p_list[i], x)}).collect();
            pixels.push(row);
        }
        let g = Vector3::new(0.0, 0.0, -9.81);
        let res = trajectory_residual(&params, &p_list, &pixels, n_timesteps, 1.0, 0.04, g, 1.0, 1.0);
        assert_eq!(res.len(), n_res);
    }

    #[tokio::test]
    async fn test_optimize_shape() {
        let n_timesteps = 5;
        let n_cams = 2;
        let dt = 0.05;
        let g = Vector3::new(0.0, 0.0, -9.81);
        let drag_true = 0.0_f64;
        let p_list = two_cameras();
        let z0 = Vector3::new(0.0, 0.0, 8.0);
        let x_vars: Vec<Vector3<f64>> = (0..n_timesteps).map(|t| {let s = t as f64 * dt; z0 + 0.5 * g * s * s}).collect();
        let pixels: Vec<Vec<Vector2<f64>>> = (0..n_cams).map(|i| (0..n_timesteps).map(|t| project_point(p_list[i], x_vars[t])).collect()).collect();

        let (x_opt, cov, drag_opt, success) = optimize_trajectory(&p_list, &pixels, dt, Some(g), 0.0, 1.0, 1.0, 1.0).await;

        assert_eq!(x_opt.len(), n_timesteps);
        let n_x = 3 * n_timesteps;
        assert_eq!(cov.nrows(), n_x);
        assert_eq!(cov.ncols(), n_x);
        assert!(drag_opt.is_finite());
        for t in 0..n_timesteps {
            assert_vec3_close(x_opt[t], x_vars[t], 1e-5);
        }
        assert_allclose_f64(drag_opt, drag_true, 0.0, 1e-4);
    }

    #[tokio::test]
    async fn test_recovers_trajectory() {
        let mut rng = StdRng::seed_from_u64(43);
        let scene = main_style_scene(&mut rng, 3, 25, 0.04, Vector3::new(0.0, 0.0, -9.81), 0.2, 2.0);
        let (x_opt, cov, drag_opt, success) = optimize_trajectory(&scene.p_list, &scene.pixels, scene.dt, Some(scene.g), scene.drag_coef, 1.0, 1.0, 1.0).await;
        let mut reproj_sq = Vec::new();
        for t in 0..scene.n_timesteps {
            for i in 0..scene.n_cameras {
                let d = project_point(scene.p_list[i], x_opt[t]) - scene.pixels[i][t];
                reproj_sq.push(d.dot(&d));
            }
        }
        let mean_reproj = (reproj_sq.iter().sum::<f64>() / reproj_sq.len() as f64).sqrt();
        assert!(mean_reproj < 12.0, "mean_reproj={mean_reproj}");
        let mean_err: f64 = (0..scene.n_timesteps)
            .map(|t| (x_opt[t] - scene.traj_true[t]).norm())
            .sum::<f64>()
            / scene.n_timesteps as f64;
        assert!(mean_err < 2.5);
        assert_allclose_f64(drag_opt, scene.drag_coef, 0.35, 0.12);
        assert!(cov.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn test_recovers_noise() {
        let mut rng = StdRng::seed_from_u64(101);
        let scene = main_style_scene(&mut rng, 3, 25, 0.04, Vector3::new(0.0, 0.0, -9.81), 0.2, 0.75);
        let (x_opt, cov, drag_opt, _success) = optimize_trajectory(&scene.p_list, &scene.pixels, scene.dt, Some(scene.g), scene.drag_coef, 1.0, 1.0, 1.0).await;
        let mean_reproj: f64 = {
            let mut acc = Vec::new();
            for t in 0..scene.n_timesteps {
                for i in 0..scene.n_cameras {
                    let d = project_point(scene.p_list[i], x_opt[t]) - scene.pixels[i][t];
                    acc.push(d.dot(&d));
                }
            }
            (acc.iter().sum::<f64>() / acc.len() as f64).sqrt()
        };
        assert!(mean_reproj < 5.0, "mean_reproj={mean_reproj}");
        let mean_err: f64 = (0..scene.n_timesteps).map(|t| (x_opt[t] - scene.traj_true[t]).norm()).sum::<f64>() / scene.n_timesteps as f64;
        assert!(mean_err < 1.2);
        assert_allclose_f64(drag_opt, scene.drag_coef, 0.35, 0.15);
        assert!(cov.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn test_recovers_two_cameras() {
        let mut rng = StdRng::seed_from_u64(202);
        let scene = main_style_scene(&mut rng, 2, 25, 0.04, Vector3::new(0.0, 0.0, -9.81), 0.2, 1.5);
        let (x_opt, cov, drag_opt, _success) = optimize_trajectory(&scene.p_list, &scene.pixels, scene.dt, Some(scene.g), scene.drag_coef, 1.0, 1.0, 1.0).await;
        let mean_err: f64 = (0..scene.n_timesteps).map(|t| (x_opt[t] - scene.traj_true[t]).norm()).sum::<f64>() / scene.n_timesteps as f64;
        assert!(mean_err < 2.0);
        assert_allclose_f64(drag_opt, scene.drag_coef, 0.35, 0.15);
        assert!(cov.iter().all(|x| x.is_finite()));
    }

    #[tokio::test]
    async fn test_recovers_high_drag() {
        let mut rng = StdRng::seed_from_u64(303);
        let scene = main_style_scene(&mut rng, 3, 25, 0.04, Vector3::new(0.0, 0.0, -9.81), 0.45, 1.25);
        let (x_opt, cov, drag_opt, _success) = optimize_trajectory(&scene.p_list, &scene.pixels, scene.dt, Some(scene.g), scene.drag_coef, 1.0, 1.0, 1.0).await;
        let mean_reproj: f64 = {
            let mut acc = Vec::new();
            for t in 0..scene.n_timesteps {
                for i in 0..scene.n_cameras {
                    let d = project_point(scene.p_list[i], x_opt[t]) - scene.pixels[i][t];
                    acc.push(d.dot(&d));
                }
            }
            (acc.iter().sum::<f64>() / acc.len() as f64).sqrt()
        };
        assert!(mean_reproj < 10.0, "mean_reproj={mean_reproj}");
        let mean_err: f64 = (0..scene.n_timesteps).map(|t| (x_opt[t] - scene.traj_true[t]).norm()).sum::<f64>() / scene.n_timesteps as f64;
        assert!(mean_err < 1.8);
        assert_allclose_f64(drag_opt, scene.drag_coef, 0.35, 0.15);
        assert!(drag_opt.is_finite());
        assert!(cov.iter().all(|x| x.is_finite()));
    }
}
