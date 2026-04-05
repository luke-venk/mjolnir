

pub async fn run() {
    let P_list = vec![
        Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
        Matrix3x4::new(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0),
    ];
    let pixels = vec![
        Vector2::new(1.0, 1.0),
        Vector2::new(2.0, 3.0),
        Vector2::new(6.0, 4.0),
    ];
    let X = dlt_triangulation(&P_list, &pixels).await;
    println!("This is X: \n{:?}", X);

    let P_list = vec![
        Matrix3x4::new(
            -1_245.78145, 761.33342, 0.0, 24_000.00183,
            -686.836277, -215.999835, 1_100.0, 15_800.00137,
            -0.953939273, -0.299999771, 0.0, 25.000019,
        ),
        Matrix3x4::new(
            -391.564502, 5_154.90807, 0.0, 48_000.00366,
            -1_030.25442, 323.999753, 4_800.0, 17_400.00206,
            -0.953939273, 0.299999771, 0.0, 25.000019,
        ),
    ];
    let pixels = vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(0.0, 0.0),
    ];
    let X = dlt_triangulation(&P_list, &pixels).await;
    println!("This is X: \n{:?}", X);
}


pub async fn run() {
    let P_list = vec![
        Matrix3x4::new(
            -1_245.78145, 761.33342, 0.0, 24_000.00183,
            -686.836277, -215.999835, 1_100.0, 15_800.00137,
            -0.953939273, -0.299999771, 0.0, 25.000019,
        ),
        Matrix3x4::new(
            -391.564502, 5_154.90807, 0.0, 48_000.00366,
            -1_030.25442, 323.999753, 4_800.0, 17_400.00206,
            -0.953939273, 0.299999771, 0.0, 25.000019,
        ),
    ];
    let pixels: Vec<Vec<Vector2<f64>>> = vec![
        vec![Vector2::new(0.0, 0.0)],
        vec![Vector2::new(0.0, 0.0)],
    ];
    let dt = 1.0;
    let g = Some(Vector3::new(0.0, 0.0, -9.81));
    let drag = 0.0;
    let pixel_sigma = 1.0;
    let physics_sigma = 0.01;
    let omega_phys = 1.0;
    let (X_opt, _cov, _drag_opt, _success) = optimize_trajectory(&P_list, &pixels, dt, g, drag, pixel_sigma, physics_sigma, omega_phys).await;
    println!("This is X: \n{:?}", X_opt);
    println!("This is covariance: \n{:?}", _cov);
}

//==============================================
// Example usage
//==============================================

fn _camera_look_at(cam_position: Vector3<f64>, target: Vector3<f64>, up: Vector3<f64>) -> (Matrix3<f64>, Vector3<f64>) {
    let z = (target - cam_position) / (target - cam_position).norm();
    let x = up.cross(&z).normalize();
    let y = z.cross(&x).normalize();
    let R = Matrix3::new(x[0], y[0], z[0], x[1], y[1], z[1], x[2], y[2], z[2]);
    let t = -R * cam_position;
    (R, t)
}

pub async fn run() {
    println!("Starting Physics-Informed Triangulation (no GCPs, fixed cameras)");

    let n_cameras = 3usize;
    let max_timesteps = 25usize;
    let dt = 0.04_f64;
    let g = Vector3::new(0.0, 0.0, -9.81);
    let drag_coef = 0.2_f64;
    let pixel_sigma = 1.0_f64;
    let physics_sigma = 0.05_f64;
    let omega_phys = 1.0_f64;

    let k_intrinsic = Matrix3::new(800.0, 0.0, 0.0, 0.0, 800.0, 0.0, 0.0, 0.0, 1.0);
    let K_list = vec![k_intrinsic; n_cameras];

    let center = Vector3::new(0.0, 0.0, 0.0);
    let up = Vector3::new(0.0, 0.0, 1.0);
    let radius = 4.0_f64;
    let height = 2.0_f64;
    let angles = [
        0.0_f64,
        2.0 * std::f64::consts::PI / 3.0,
        4.0 * std::f64::consts::PI / 3.0,
    ];
    let cam_positions = Matrix3::new(
        radius * angles[0].cos(),
        radius * angles[0].sin(),
        height,
        radius * angles[1].cos(),
        radius * angles[1].sin(),
        height,
        radius * angles[2].cos(),
        radius * angles[2].sin(),
        height,
    );

    let mut R_list = Vec::with_capacity(n_cameras);
    let mut t_list = Vec::with_capacity(n_cameras);
    for i in 0..n_cameras {
        let pos_i = Vector3::new(
            cam_positions[(i, 0)],
            cam_positions[(i, 1)],
            cam_positions[(i, 2)],
        );
        let (r, t) = _camera_look_at(pos_i, center, up);
        R_list.push(r);
        t_list.push(t);
    }

    let mut P_list = Vec::with_capacity(n_cameras);
    for i in 0..n_cameras {
        let k = K_list[i];
        let r = R_list[i];
        let t = t_list[i];
        let rt = Matrix3x4::new(
            r[(0, 0)],
            r[(0, 1)],
            r[(0, 2)],
            t[0],
            r[(1, 0)],
            r[(1, 1)],
            r[(1, 2)],
            t[1],
            r[(2, 0)],
            r[(2, 1)],
            r[(2, 2)],
            t[2],
        );
        P_list.push(k * rt);
    }

    let x0 = Vector3::new(0.0, 0.0, 1.5);
    let v0 = Vector3::new(1.0, 1.0, 2.5);
    let mut traj_true: Vec<Vector3<f64>> = Vec::new();
    let mut x = x0;
    let mut v = v0;
    for _ in 0..max_timesteps {
        traj_true.push(x);
        let acc = g - drag_coef * v;
        v = v + acc * dt;
        x = x + v * dt;
        if x[2] < 0.2 {
            break;
        }
    }
    let n_timesteps = traj_true.len();

    let mut pixels: Vec<Vec<Vector2<f64>>> = Vec::new();
    for i in 0..n_cameras {
        let mut pixel_t = Vec::with_capacity(n_timesteps);
        for t in 0..n_timesteps {
            let pred = project_point(P_list[i], traj_true[t]);
            pixel_t.push(pred);
        }
        pixels.push(pixel_t);
    }

    let time_start = Instant::now();
    let (x_opt, cov, drag_opt, success) = optimize_trajectory(
        &P_list,
        &pixels,
        dt,
        Some(g),
        drag_coef,
        pixel_sigma,
        physics_sigma,
        omega_phys,
    )
    .await;
    let time_end = Instant::now();

    println!("Time taken: {:.4} s", time_end.duration_since(time_start).as_secs_f64());
    println!("Optimized trajectory X_opt:\n{:?}", x_opt);
    println!("Covariance matrix shape: {} x {}", cov.nrows(), cov.ncols());

    // Ground-truth trajectory
    let mut file = File::create("trajectory_true.csv").expect("create trajectory_true.csv");
    for p in &traj_true {
        writeln!(file, "{},{},{}", p[0], p[1], p[2]).expect("write trajectory");
    }

    // Optimized trajectory
    let mut file = File::create("trajectory_optimized.csv").expect("create trajectory_optimized.csv");
    for p in &x_opt {
        writeln!(file, "{},{},{}", p[0], p[1], p[2]).expect("write trajectory");
    }

    // Full covariance as CSV (rows)
    let mut file = File::create("covariance.csv").expect("create covariance.csv");
    for i in 0..cov.nrows() {
        let row: Vec<String> = (0..cov.ncols())
            .map(|j| format!("{}", cov[(i, j)]))
            .collect();
        writeln!(file, "{}", row.join(",")).expect("write cov row");
    }
}