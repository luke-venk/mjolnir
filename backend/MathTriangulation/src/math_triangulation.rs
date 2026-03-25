use nalgebra::{Matrix3, Matrix3x4, Vector3, Vector2, Vector4, DMatrix, DVector};
use nalgebra::linalg::SVD;
use nalgebra::base::VecStorage;
use nalgebra::dimension::{Dyn, U1};
use levenberg_marquardt::{LeastSquaresProblem, LevenbergMarquardt};
use std::fs::File;
use std::io::Write;
use std::time::Instant;


/// Helper function to project a 3D point onto a 2D image plane.
fn project_point(P: Matrix3x4<f64>, x: Vector3<f64>) -> Vector2<f64> {
    let x_h = Vector4::new(x[0], x[1], x[2], 1.0);
    let x_proj = P * x_h;
    Vector2::new(x_proj[0] / x_proj[2], x_proj[1] / x_proj[2])
}

/// This function does initial estimate of the 3D position using the DLT algorithm.
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

///Setup the problem for the trajectory of a projectile. 
///This includes the number of position parameters, the drag coefficient, the number of residuals, and the numerical Jacobian.
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

/// Function sets up and returns the trajectory of a projectile using the Levenberg-Marquardt algorithm.
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
        let drag_k = self.drag();
        for t in 1..self.n_timesteps.saturating_sub(1) {
            let X_prev = Vector3::new(self.params[(t - 1) * 3], self.params[(t - 1) * 3 + 1], self.params[(t - 1) * 3 + 2]);
            let X_curr = Vector3::new(self.params[t * 3], self.params[t * 3 + 1], self.params[t * 3 + 2]);
            let X_next = Vector3::new(self.params[(t + 1) * 3], self.params[(t + 1) * 3 + 1], self.params[(t + 1) * 3 + 2]);
            let phys_res = (X_next - 2.0 * X_curr + X_prev - self.g * self.dt * self.dt - Vector3::new(drag_k, drag_k, drag_k) * self.dt * self.dt) / self.physics_sigma;
            residuals.push(self.omega_phys * phys_res[0]);
            residuals.push(self.omega_phys * phys_res[1]);
            residuals.push(self.omega_phys * phys_res[2]);
        }
        Some(DVector::from_vec(residuals))
    }

    fn jacobian(&self) -> Option<DMatrix<f64>> {
        None
    }
}

/// This function optimizes the trajectory of a projectile using the Levenberg-Marquardt algorithm.
/// Initial guess for the trajectory is gotten from DLT triangulation.
/// Then to further optimize the trajectory, we use the Levenberg-Marquardt algorithm.
/// We return the optimized trajectory and the covariance matrix.
/// Args:
///     P_list: List of camera projection matrices. Should be gotten from camera calibration.
///     pixels: List of pixel coordinates for each camera at each timestep. Should be gotten from CV.
///     dt: Time step (1/fps).
///     g: Gravity vector.
///     drag: Drag coefficient.
///     pixel_sigma: Standard deviation of the pixel noise.
///     physics_sigma: Standard deviation of the physics noise.
///     omega_phys: Hyperparameter for the physics noise.
/// Returns:
///     Tuple containing the optimized trajectory and the covariance matrix.
async fn optimize_trajectory(P_list: &[Matrix3x4<f64>], pixels: &[Vec<Vector2<f64>>], dt: f64, g: Option<Vector3<f64>>, drag: f64, pixel_sigma: f64, physics_sigma: f64, omega_phys: f64) -> (Vec<Vector3<f64>>, DMatrix<f64>) {
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
    let (problem, _report) = LevenbergMarquardt::default().minimize(problem);
    let X_opt: Vec<Vector3<f64>> = (0..n_timesteps)
        .map(|t| Vector3::new(problem.params[t * 3], problem.params[t * 3 + 1], problem.params[t * 3 + 2]))
        .collect();
    let jacobian = problem.numerical_jacobian(1e-7);
    let cov_full = problem.covariance_from_jacobian(&jacobian).unwrap_or_else(|| DMatrix::zeros(3 * n_timesteps + 1, 3 * n_timesteps + 1));
    let cov = cov_full.view((0, 0), (3 * n_timesteps, 3 * n_timesteps)).into_owned();
    (X_opt, cov)
}


pub async fn run() {
}