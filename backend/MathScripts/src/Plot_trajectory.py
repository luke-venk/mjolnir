"""
Basic trajectory plotting script.
Does not need to be turned into Rust code. Simply for validating the rust code compared to the python code.
"""

import numpy as np
import matplotlib.pyplot as plt
from mpl_toolkits.mplot3d import Axes3D

def plot_3d_trajectory(X_opt, X_true=None, cov=None, out_path="trajectory_3d.png"):
    try:
        import matplotlib.pyplot as plt
        from mpl_toolkits.mplot3d import Axes3D
    except ImportError:
        print("Install matplotlib to plot: pip install matplotlib")
        return
    n = len(X_opt)
    fig = plt.figure(figsize=(10, 6))
    ax = fig.add_subplot(111, projection="3d")

    frame_index = np.arange(n)
    sc = ax.scatter(X_opt[:, 0], X_opt[:, 1], X_opt[:, 2], c=frame_index, cmap="viridis", s=200, edgecolors="none")
    if X_true is not None:
        ax.plot(X_true[:, 0], X_true[:, 1], X_true[:, 2], "b-", alpha=0.25, linewidth=1.5)
    else:
        print("No true trajectory provided, only plotting optimized trajectory")
    ax.plot(X_opt[:, 0], X_opt[:, 1], X_opt[:, 2], "r-", alpha=1.0, linewidth=1.5)


    # Optional: Plot covariance ellipsoids
    #if cov is not None and np.all(np.isfinite(cov)):
    #    max_std = np.max(np.sqrt(np.diag(cov)))
    #    scale = 0.3 / max_std if max_std > 1.0 else 1.0
    #    for t in range(0, n, max(1, n // 15)):
    #        cov_t = cov[3 * t : 3 * t + 3, 3 * t : 3 * t + 3]
    #        if np.any(np.isnan(cov_t)) or np.any(np.linalg.eigvalsh(cov_t) <= 0):
    #            continue
    #        eigs, Q = np.linalg.eigh(cov_t)
    #        eigs = np.maximum(eigs, 1e-12)
    #        radii = scale * np.sqrt(eigs)
    #        u = np.linspace(0, 2 * np.pi, 20)
    #        v = np.linspace(0, np.pi, 20)
    #        x = np.outer(np.cos(u), np.sin(v))
    #        y = np.outer(np.sin(u), np.sin(v))
    #        z = np.outer(np.ones_like(u), np.cos(v))
    #        pts = np.column_stack([x.ravel(), y.ravel(), z.ravel()]) @ (Q * radii).T + X_opt[t]
    #        ax.scatter(pts[:, 0], pts[:, 1], pts[:, 2], color="gray", alpha=0.15, s=1)
    ax.set_xlabel("x (m)", fontsize=20)
    ax.set_ylabel("y (m)", fontsize=20)
    ax.set_zlabel("z (m)", fontsize=20)
    cbar = fig.colorbar(sc, ax=ax, shrink=0.6)
    cbar.set_label("Frame", fontsize=20)
    plt.tight_layout()
    plt.savefig(out_path, dpi=120)
    print(f"Saved: {out_path}")
    plt.show()


if __name__ == "__main__":
    X_opt = np.loadtxt("trajectory_optimized.csv", delimiter=",")
    X_true = np.loadtxt("trajectory_true.csv", delimiter=",")
    cov = np.loadtxt("covariance.csv", delimiter=",")
    plot_3d_trajectory(X_opt, X_true=X_true, cov=cov, out_path="trajectory_3d.png")