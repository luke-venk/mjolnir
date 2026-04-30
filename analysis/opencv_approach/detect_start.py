import cv2
import numpy as np
import matplotlib.pyplot as plt
from pathlib import Path
from scipy.interpolate import CubicSpline
from tuned_cv1k import extract_trajectory_from_video

plt.rcParams['font.size'] = 18
def _natural_cubic_spline_second_derivatives(t, y):
    n = len(t)
    if n < 3:
        return np.zeros(n, dtype=float)

    h = np.diff(t)
    alpha = np.zeros(n, dtype=float)
    alpha[1:-1] = (3.0 / h[1:]) * (y[2:] - y[1:-1]) - (3.0 / h[:-1]) * (y[1:-1] - y[:-2])

    l = np.ones(n, dtype=float)
    mu = np.zeros(n, dtype=float)
    z = np.zeros(n, dtype=float)

    for i in range(1, n - 1):
        l[i] = 2.0 * (t[i + 1] - t[i - 1]) - h[i - 1] * mu[i - 1]
        mu[i] = h[i] / l[i]
        z[i] = (alpha[i] - h[i - 1] * z[i - 1]) / l[i]

    c = np.zeros(n, dtype=float)
    for j in range(n - 2, -1, -1):
        c[j] = z[j] - mu[j] * c[j + 1]

    return c


def _evaluate_cubic_spline(t, y, c, t_eval):
    y_eval = np.empty_like(t_eval, dtype=float)
    n = len(t)

    for idx, x_val in enumerate(t_eval):
        if x_val <= t[0]:
            i = 0
        elif x_val >= t[-1]:
            i = n - 2
        else:
            i = np.searchsorted(t, x_val) - 1

        h = t[i + 1] - t[i]
        if h == 0:
            y_eval[idx] = y[i]
            continue

        a = (t[i + 1] - x_val) / h
        b = (x_val - t[i]) / h
        y_eval[idx] = (
            a * y[i]
            + b * y[i + 1]
            + ((a**3 - a) * c[i] + (b**3 - b) * c[i + 1]) * (h**2) / 6.0
        )

    return y_eval


def _fit_cubic_spline(points, num_samples=200):
    points = np.asarray(points, dtype=float)
    if points.shape[0] < 2:
        return points

    t = np.linspace(0.0, 1.0, points.shape[0])
    x = points[:, 0]
    y = points[:, 1]
    c_x = _natural_cubic_spline_second_derivatives(t, x)
    c_y = _natural_cubic_spline_second_derivatives(t, y)

    t_sample = np.linspace(0.0, 1.0, num_samples)
    x_sample = _evaluate_cubic_spline(t, x, c_x, t_sample)
    y_sample = _evaluate_cubic_spline(t, y, c_y, t_sample)

    return np.vstack([x_sample, y_sample]).T


def interpolate_at_same_time_intervals(trajectory1, trajectory2, num_points=100):
    """
    Interpolate both trajectories at the same normalized time intervals.
    
    Args:
        trajectory1: numpy array of shape (N1, 2) - first trajectory points
        trajectory2: numpy array of shape (N2, 2) - second trajectory points
        num_points: number of interpolated points to generate for each trajectory
    
    Returns:
        interp1: interpolated points for trajectory 1 at same time intervals
        interp2: interpolated points for trajectory 2 at same time intervals
    """
    # Create normalized time parameter (0 to 1) for each trajectory
    t1 = np.linspace(0.0, 1.0, len(trajectory1))
    t2 = np.linspace(0.0, 1.0, len(trajectory2))
    
    # Create common time points
    t_common = np.linspace(0.0, 1.0, num_points)
    
    # Fit cubic splines for trajectory 1
    cs_x1 = CubicSpline(t1, trajectory1[:, 0], bc_type='natural')
    cs_y1 = CubicSpline(t1, trajectory1[:, 1], bc_type='natural')
    
    # Fit cubic splines for trajectory 2
    cs_x2 = CubicSpline(t2, trajectory2[:, 0], bc_type='natural')
    cs_y2 = CubicSpline(t2, trajectory2[:, 1], bc_type='natural')
    
    # Evaluate at common time points
    interp1 = np.column_stack([cs_x1(t_common), cs_y1(t_common)])
    interp2 = np.column_stack([cs_x2(t_common), cs_y2(t_common)])
    
    return interp1, interp2, t_common


def plot_two_throws_with_common_interpolation(video_path1: str, video_path2: str, 
                                              output_path: str = "throw_comparison_with_interpolation.png",
                                              num_interp_points=100):
    """
    Plot two throw trajectories with cubic spline interpolation at the same time intervals.
    Shows both the original tracked points and the interpolated points at common times.
    """
    # Extract trajectories
    positions1, detected1, fps1, _, trail1 = extract_trajectory_from_video(video_path1)
    positions2, detected2, fps2, _, trail2 = extract_trajectory_from_video(video_path2)

    tracked1 = np.asarray(trail1, dtype=float)
    tracked2 = np.asarray(trail2, dtype=float)

    t1_trail = np.arange(len(trail1)) * 1/fps1
    t2_trail = np.arange(len(trail2)) * 1/fps2


    if tracked1.size == 0 or tracked2.size == 0:
        print("One of the throws has no trajectory data.")
        return

    # Filter detection status
    def filter_detection_status(positions, detected, tracked):
        if len(positions) == len(detected):
            filtered = np.asarray([d for p, d in zip(positions, detected) if p is not None], dtype=bool)
            if len(filtered) != len(tracked):
                return np.ones(len(tracked), dtype=bool)
            return filtered
        return np.ones(len(tracked), dtype=bool)

    detected1 = filter_detection_status(positions1, detected1, tracked1)
    detected2 = filter_detection_status(positions2, detected2, tracked2)

    # Interpolate both trajectories at the same normalized time intervals
    interp1, interp2, t_common = interpolate_at_same_time_intervals(tracked1, tracked2, num_interp_points)
    
    # Calculate distance between corresponding interpolated points
    distances = np.sqrt(np.sum((interp1 - interp2)**2, axis=1))
    
    # Create figure with subplots
    fig = plt.figure(figsize=(20, 10))
    
    # Main trajectory plot
    ax1 = plt.subplot(2, 2, (1, 2))
    
    # Plot the full spline trajectories
    spline1_full = _fit_cubic_spline(tracked1, num_samples=200)
    spline2_full = _fit_cubic_spline(tracked2, num_samples=200)
    
    # ax1.plot(spline1_full[:, 0], spline1_full[:, 1], 'r-', linewidth=2.5, alpha=0.7, label='Throw 1 trajectory')
    # ax1.plot(spline2_full[:, 0], spline2_full[:, 1], 'b-', linewidth=2.5, alpha=0.7, label='Throw 2 trajectory')
    
    # Plot original tracked points
    ax1.scatter(tracked1[detected1, 0], tracked1[detected1, 1], 
               c='red', s=30, alpha=0.5, label='Throw 1 detected')
    ax1.scatter(tracked2[detected2, 0], tracked2[detected2, 1], 
               c='blue', s=30, alpha=0.5, label='Throw 2 detected')
    
    # Plot interpolated points at common times
    # Color by normalized time (t_common)
    # scatter1 = ax1.scatter(interp1[:, 0], interp1[:, 1], 
    #                       c=t_common, cmap='RdYlGn', s=60, 
    #                       marker='o', edgecolors='black', linewidth=1.5,
    #                       label='Throw 1 interpolated', vmin=0, vmax=1)
    # scatter2 = ax1.scatter(interp2[:, 0], interp2[:, 1], 
    #                       c=t_common, cmap='RdYlGn', s=60, 
                        #   marker='s', edgecolors='black', linewidth=1.5,
                        #   label='Throw 2 interpolated', vmin=0, vmax=1)
    
    # Connect corresponding points with lines
    # for i in range(0, len(interp1), max(1, num_interp_points // 20)):  # Show every Nth line to avoid clutter
    #     ax1.plot([interp1[i, 0], interp2[i, 0]], 
    #             [interp1[i, 1], interp2[i, 1]], 
    #             'gray', linestyle='--', alpha=0.3, linewidth=0.8)
    
    # Mark start and end points
    ax1.scatter(tracked1[0, 0], tracked1[0, 1], c='darkred', s=200, 
               marker='*', zorder=5, edgecolors='black', linewidth=2)
    ax1.scatter(tracked1[-1, 0], tracked1[-1, 1], c='red', s=200, 
               marker='*', zorder=5, edgecolors='black', linewidth=2)
    ax1.scatter(tracked2[0, 0], tracked2[0, 1], c='darkblue', s=200, 
               marker='*',zorder=5, edgecolors='black', linewidth=2)
    ax1.scatter(tracked2[-1, 0], tracked2[-1, 1], c='blue', s=200, 
               marker='*', zorder=5, edgecolors='black', linewidth=2)
    
    ax1.set_xlabel('X (pixels)', fontsize=20)
    ax1.set_ylabel('Y (pixels)', fontsize=20)
    ax1.set_title('Throw Comparison from Different Cameras', fontsize=24)
    ax1.invert_yaxis()
    ax1.grid(True, alpha=0.3)
    ax1.legend(loc='best', fontsize=16)
    
    # Add colorbar for normalized time
    # cbar = plt.colorbar(scatter1, ax=ax1)
    # cbar.set_label('Normalized Time (0=start, 1=end)', fontsize=10)
   
    
    # Add statistics
    mean_distance = np.mean(distances)
    max_distance = np.max(distances)
    min_distance = np.min(distances)
    final_distance = distances[-1]

    
    # Trajectory information table
    ax2 = plt.subplot(2, 2, 4)
    ax2.axis('tight')
    ax2.axis('off')
    
    # Prepare statistics
    stats_data = [
        ['Metric', 'Throw 1', 'Throw 2'],
        ['Total frames', f'{len(tracked1)}', f'{len(tracked2)}'],
        ['Duration (s)', f'{len(tracked1)/fps1:.2f}', f'{len(tracked2)/fps2:.2f}'],
        ['X range', f'{tracked1[:,0].min():.1f}-{tracked1[:,0].max():.1f}', 
         f'{tracked2[:,0].min():.1f}-{tracked2[:,0].max():.1f}'],
        ['Y range', f'{tracked1[:,1].min():.1f}-{tracked1[:,1].max():.1f}', 
         f'{tracked2[:,1].min():.1f}-{tracked2[:,1].max():.1f}'],
        ['Detected %', f'{np.sum(detected1)/len(detected1)*100:.1f}%', 
         f'{np.sum(detected2)/len(detected2)*100:.1f}%'],
    ]
    
    # Create table
    table = ax2.table(cellText=stats_data, loc='center', cellLoc='center')
    table.auto_set_font_size(False)
    table.set_fontsize(10)
    table.scale(1, 1.5)
    
    # Style the table
    for i in range(len(stats_data)):
        for j in range(len(stats_data[0])):
            if i == 0:
                table[(i, j)].set_facecolor('#40466e')
                table[(i, j)].set_text_props(weight='bold', color='white')
            else:
                if j == 0:
                    table[(i, j)].set_facecolor('#e6e6e6')
                else:
                    table[(i, j)].set_facecolor('#f5f5f5')
    
    ax2.set_title('Trajectory Statistics', fontsize=20, pad=20)
    
    plt.suptitle(f'Throw Comparison with {num_interp_points} Common-Time Interpolation Points\n'
                f'Line connecting corresponding time points | Color indicates normalized time',
                fontsize=24, fontweight='bold')
    
    plt.tight_layout()
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    plt.show()
    
    print(f"\n=== Interpolation Results ===")
    print(f"Number of common interpolation points: {num_interp_points}")
    print(f"Mean distance between trajectories: {mean_distance:.2f} pixels")
    print(f"Max distance between trajectories: {max_distance:.2f} pixels")
    print(f"Min distance between trajectories: {min_distance:.2f} pixels")
    print(f"Final separation distance: {final_distance:.2f} pixels")
    print(f"Comparison plot saved to: {output_path}")
    
    return interp1, interp2, t_common, distances, t1_trail, t2_trail, trail1, trail2
import numpy as np
import matplotlib.pyplot as plt

def find_throw_start_stable(acc_s, threshold=1e4, window=15, spike_tol=3e4):
    """
    Detect first stable low-acceleration region.

    Conditions:
    - acceleration stays below threshold for `window` frames
    - no large spikes after start (sanity check)
    """

    acc_s = np.asarray(acc_s)

    below = acc_s < threshold

    for i in range(len(acc_s) - window):
        segment = acc_s[i:i + window]

        # must be consistently low
        if np.all(segment < threshold):
            
            # check for spikes AFTER candidate start
            post = acc_s[i:]

            if np.max(post) < spike_tol:
                return i

    return None

def smooth(signal, k=5):
    kernel = np.ones(k) / k
    return np.convolve(signal, kernel, mode='same')

def find_constant_accel_region(acc_mag, tol, window=5):
    dacc = np.abs(np.diff(acc_mag))
    
    for i in range(len(dacc) - window):
        segment = dacc[i:i+window]
        if np.all(segment < tol):
            return i  # start index of constant region
    return None

def a(x, t):
    """Calculate acceleration from position and time"""
    x = np.asarray(x, dtype=float)
    t = np.asarray(t, dtype=float)

    a_out = []

    for i in range(1, len(x) - 1):
        dt1 = t[i] - t[i-1]
        dt2 = t[i+1] - t[i]

        if dt1 == 0 or dt2 == 0:
            a_out.append(np.array([np.nan, np.nan]))
            continue

        v1 = (x[i] - x[i-1]) / dt1
        v2 = (x[i+1] - x[i]) / dt2

        ai = 2 * (v2 - v1) / (dt1 + dt2)
        a_out.append(ai)

    return np.array(a_out)

def jerk(x, t):
    """Calculate jerk (3rd derivative) from position and time"""
    x = np.asarray(x, dtype=float)
    t = np.asarray(t, dtype=float)
    
    # First get acceleration
    a_vals = a(x, t)
    
    if len(a_vals) < 2:
        return np.array([])
    
    # Calculate jerk from acceleration
    jerk_out = []
    
    # Need to align indices properly
    # a[i] corresponds to x[i+1] roughly, so we need to map times
    for i in range(1, len(a_vals) - 1):
        # Get times for acceleration points
        # a_vals[i] corresponds to position index i+1
        idx = i + 1  # position index for this acceleration
        
        dt1 = t[idx] - t[idx-1]
        dt2 = t[idx+1] - t[idx]
        
        if dt1 == 0 or dt2 == 0:
            jerk_out.append(np.array([np.nan, np.nan]))
            continue
        
        # Get jerk as derivative of acceleration
        j = (a_vals[i] - a_vals[i-1]) / ((dt1 + dt2) / 2)
        jerk_out.append(j)
    
    return np.array(jerk_out)

def find_throw_start_from_jerk(jerk_mag, jerk_threshold, min_region_frames=10):
    """
    Find the smallest frame index where all subsequent frames have jerk below threshold
    
    Parameters:
    -----------
    jerk_mag : array of jerk magnitudes
    jerk_threshold : maximum allowed jerk
    min_region_frames : minimum number of frames that must remain below threshold
    
    Returns:
    --------
    start_idx : first frame index where all subsequent frames meet jerk criteria
    """
    if len(jerk_mag) == 0:
        return None
    
    # Work backwards from the end to find the longest valid suffix
    # Start from the end and find where the condition breaks
    valid_suffix_start = len(jerk_mag)
    
    for i in range(len(jerk_mag) - 1, -1, -1):
        if jerk_mag[i] < jerk_threshold:
            valid_suffix_start = i
        else:
            break
    
    # Check if we have enough frames
    if len(jerk_mag) - valid_suffix_start < min_region_frames:
        return None
    
    # Return the first index of the valid suffix
    # Need to adjust because jerk array is shifted relative to original frames
    # Jerk at index i corresponds approximately to frame i+2
    start_frame = valid_suffix_start + 2
    
    return start_frame

def find_constant_accel_region_with_jerk_check(acc_s, jerk_mag, accel_tol, jerk_threshold, window=5):
    """
    Find region with constant acceleration AND low jerk, starting from the earliest
    point where all subsequent jerk values are below threshold
    """
    # First, find the earliest frame where all subsequent jerk is below threshold
    jerk_start = find_throw_start_from_jerk(jerk_mag, jerk_threshold)
    
    if jerk_start is None:
        return None, None, None
    
    # Now verify that acceleration is also constant from that point
    # Check if acceleration stays within tolerance from jerk_start onward
    if jerk_start >= len(acc_s):
        return None, None, None
    
    # Find the constant acceleration region starting from jerk_start
    start_idx = jerk_start
    end_idx = jerk_start
    
    # Expand forward while acceleration change is small and jerk is low
    while end_idx < len(acc_s) - 1:
        # Check next point
        if abs(acc_s[end_idx + 1] - acc_s[end_idx]) < accel_tol:
            # Check jerk at corresponding position
            jerk_idx = min(end_idx, len(jerk_mag) - 1)
            if jerk_idx >= 0 and jerk_mag[jerk_idx] < jerk_threshold:
                end_idx += 1
            else:
                break
        else:
            break
    
    # Check if we have a valid region (at least window size)
    if end_idx - start_idx < window:
        return None, None, None
    
    # Verify entire region has low jerk
    is_valid, jerk_stats = check_constant_jerk_region(
        acc_s, jerk_mag, start_idx, end_idx, jerk_threshold
    )
    
    if not is_valid:
        print(f"Warning: Region fails jerk test. Jerk stats: {jerk_stats}")
        return None, None, None
    
    return start_idx, end_idx, jerk_stats

def check_constant_jerk_region(acc_s, jerk_mag, start_idx, end_idx, jerk_threshold):
    """
    Verify that jerk remains close to zero in the constant acceleration region
    
    Parameters:
    -----------
    acc_s : smoothed acceleration magnitude
    jerk_mag : jerk magnitude
    start_idx : start index of constant acceleration region
    end_idx : end index of constant acceleration region
    jerk_threshold : maximum allowed jerk magnitude
    
    Returns:
    --------
    is_valid : bool, whether the region has constant acceleration
    jerk_stats : dict with statistics about jerk in the region
    """
    if start_idx is None or end_idx is None:
        return False, {}
    
    # Adjust indices for jerk (jerk array is shorter)
    # Map acceleration indices to jerk indices
    jerk_start = max(0, start_idx - 2)
    jerk_end = min(len(jerk_mag), end_idx - 1)
    
    if jerk_start >= jerk_end:
        return False, {}
    
    jerk_region = jerk_mag[jerk_start:jerk_end]
    
    if len(jerk_region) == 0:
        return False, {}
    
    jerk_stats = {
        'mean_jerk': np.mean(jerk_region),
        'std_jerk': np.std(jerk_region),
        'max_jerk': np.max(jerk_region),
        'min_jerk': np.min(jerk_region),
        'pct_below_threshold': 100 * np.sum(jerk_region < jerk_threshold) / len(jerk_region)
    }
    
    # Check if jerk is small throughout the region
    is_valid = np.all(jerk_region < jerk_threshold)
    
    # Also verify that all subsequent frames after start meet the condition
    if is_valid and jerk_start > 0:
        # Check all frames after start
        all_subsequent = jerk_mag[jerk_start:]
        all_valid = np.all(all_subsequent < jerk_threshold)
        if not all_valid:
            is_valid = False
            jerk_stats['note'] = 'Later frames violate jerk threshold'
    
    return is_valid, jerk_stats

def calculate_throw_start(x1, x2, t1, t2, accel_tol=1e4, jerk_tol=1e5):
    """
    Calculate throw start using jerk threshold - selects smallest frame index
    where all subsequent frames have jerk below threshold
    """
    x1 = np.asarray(x1)
    x2 = np.asarray(x2)

    # Calculate acceleration
    a1 = a(x1, t1)
    a2 = a(x2, t2)
    
    # Calculate jerk
    j1 = jerk(x1, t1)
    j2 = jerk(x2, t2)

    a1_mag = np.linalg.norm(a1, axis=1)
    a2_mag = np.linalg.norm(a2, axis=1)
    
    j1_mag = np.linalg.norm(j1, axis=1) if len(j1) > 0 else np.array([])
    j2_mag = np.linalg.norm(j2, axis=1) if len(j2) > 0 else np.array([])

    # Smooth acceleration and jerk
    a1_s = smooth(a1_mag, k=7)
    a2_s = smooth(a2_mag, k=7)
    
    j1_s = smooth(j1_mag, k=5) if len(j1_mag) > 0 else np.array([])
    j2_s = smooth(j2_mag, k=5) if len(j2_mag) > 0 else np.array([])
    
    # Plot acceleration analysis
    fig, axes = plt.subplots(2, 2, figsize=(15, 10))
    
    # Throw 1 acceleration
    axes[0, 0].plot(a1_mag, label='|a| (raw)', linestyle='--', alpha=0.7)
    axes[0, 0].plot(a1_s, label='|a| (smooth)', linewidth=2)
    axes[0, 0].set_yscale('log')
    axes[0, 0].set_title("Acceleration Analysis (Throw 1)")
    axes[0, 0].set_xlabel("Frame Index")
    axes[0, 0].set_ylabel("Acceleration (pixels/s²)")
    axes[0, 0].grid(True)
    axes[0, 0].legend()
    
    # Throw 2 acceleration
    axes[0, 1].plot(a2_mag, label='|a| (raw)', linestyle='--', alpha=0.7)
    axes[0, 1].plot(a2_s, label='|a| (smooth)', linewidth=2)
    axes[0, 1].set_yscale('log')
    axes[0, 1].set_title("Acceleration Analysis (Throw 2)")
    axes[0, 1].set_xlabel("Frame Index")
    axes[0, 1].set_ylabel("Acceleration (pixels/s²)")
    axes[0, 1].grid(True)
    axes[0, 1].legend()
    p = 2
    # Throw 1 jerk
    if len(j1_s) > 0:
        axes[1, 0].plot(j1_mag, label='|jerk| (raw)', linestyle='--', alpha=0.7)
        axes[1, 0].plot(j1_s, label='|jerk| (smooth)', linewidth=2)
        axes[1, 0].axhline(y=jerk_tol*p, color='r', linestyle='--', label=f'Jerk threshold ({jerk_tol*p})')
        axes[1, 0].set_title("Jerk Analysis (Throw 1)")
        axes[1, 0].set_xlabel("Frame Index")
        axes[1, 0].set_ylabel("Jerk (pixels/s³)")
        axes[1, 0].grid(True)
        axes[1, 0].set_yscale('log') 
        axes[1, 0].legend()
        
        # Mark the detected start based on jerk criterion
        start1_candidate = find_throw_start_from_jerk(j1_s, jerk_tol)
        if start1_candidate is not None:
            axes[1, 0].axvline(x=start1_candidate, color='g', linestyle='--', 
                              label=f'Jerk-based start ({start1_candidate})')
            axes[1, 0].legend()

    else:
        axes[1, 0].text(0.5, 0.5, 'Insufficient data for jerk calculation', 
                       ha='center', va='center', transform=axes[1, 0].transAxes)
        axes[1, 0].set_title("Jerk Analysis (Throw 1)")
    
    # Throw 2 jerk
    if len(j2_s) > 0:
        axes[1, 1].plot(j2_mag, label='|jerk| (raw)', linestyle='--', alpha=0.7)
        axes[1, 1].plot(j2_s, label='|jerk| (smooth)', linewidth=2)
        axes[1, 1].axhline(y=jerk_tol, color='r', linestyle='--', label=f'Jerk threshold ({jerk_tol})')
        axes[1, 1].set_title("Jerk Analysis (Throw 2)")
        axes[1, 1].set_xlabel("Frame Index")
        axes[1, 1].set_ylabel("Jerk (pixels/s³)")
        axes[1, 1].grid(True)
        axes[1, 1].set_yscale('log') 
        axes[1, 1].legend()
        
        # Mark the detected start based on jerk criterion
        start2_candidate = find_throw_start_from_jerk(j2_s, jerk_tol)
        if start2_candidate is not None:
            axes[1, 1].axvline(x=start2_candidate, color='g', linestyle='--', 
                              label=f'Jerk-based start ({start2_candidate})')
            axes[1, 1].legend()
    else:
        axes[1, 1].text(0.5, 0.5, 'Insufficient data for jerk calculation', 
                       ha='center', va='center', transform=axes[1, 1].transAxes)
        axes[1, 1].set_title("Jerk Analysis (Throw 2)")
    
    plt.tight_layout()
    plt.savefig('acceleration_jerk_analysis.png')
    plt.show()
    
    # Find throw start based purely on jerk criterion (all subsequent frames below threshold)
    if len(j1_s) > 0:
        start1 = find_throw_start_from_jerk(j1_s, jerk_tol*p)
        if start1 is not None:
            print(f"Throw 1: Found start at frame {start1} where all subsequent jerk < {jerk_tol}")
        else:
            print(f"Throw 1: No valid start found with all subsequent jerk < {jerk_tol}")
            # Fallback to acceleration-based method
            start1 = find_throw_start_stable(a1_s)
    else:
        start1 = find_throw_start_stable(a1_s)
        print("Warning: Cannot verify constant acceleration with jerk for Throw 1")
    
    if len(j2_s) > 0:
        start2 = find_throw_start_from_jerk(j2_s, jerk_tol)
        if start2 is not None:
            print(f"Throw 2: Found start at frame {start2} where all subsequent jerk < {jerk_tol}")
        else:
            print(f"Throw 2: No valid start found with all subsequent jerk < {jerk_tol}")
            # Fallback to acceleration-based method
            start2 = find_throw_start_stable(a2_s)
    else:
        start2 = find_throw_start_stable(a2_s)
        print("Warning: Cannot verify constant acceleration with jerk for Throw 2")
    
    return start1, start2


# Example usage
if __name__ == "__main__":
    throw0 = "throw0_lon.mp4"
    throw1 = "throw1_lon.mp4"
    
    jt = 1e6
    # Plot with common interpolation points
    interp1, interp2, t_common, distances, t1, t2, trail1, trail2 = plot_two_throws_with_common_interpolation(
        throw0, throw1, 
        output_path="throw_comparison_interpolated.png",
        num_interp_points=1
    )

    #(start1, end1), (start2, end2)
    start1,start2 = calculate_throw_start(
    trail1, trail2, t1, t2, jerk_tol=jt
    )
    end1 = None
    end2 = None

    from matplotlib import cm

    tracked1 = np.asarray(trail1)
    tracked2 = np.asarray(trail2)

    # normalized time (0 = start, 1 = end)
    t_norm1 = np.linspace(0, 1, len(tracked1))
    t_norm2 = np.linspace(0, 1, len(tracked2))

    plt.figure(figsize=(10, 8))

    # ---- Throw 1 gradient ----
    sc1 = plt.scatter(
        tracked1[:, 0], tracked1[:, 1],
        c=t_norm1,
        cmap='Reds',
        s=15,
        label='Throw 1'
    )

    # ---- Throw 2 gradient ----
    sc2 = plt.scatter(
        tracked2[:, 0], tracked2[:, 1],
        c=t_norm2,
        cmap='Blues',
        s=15,
        label='Throw 2'
    )

    # ---- START / END markers ----
    if start1 is not None:
        plt.scatter(tracked1[start1, 0], tracked1[start1, 1],
                    c='yellow', s=250, marker='o',
                    edgecolors='black', label='Start 1')

    if end1 is not None:
        plt.scatter(tracked1[end1, 0], tracked1[end1, 1],
                    c='orange', s=250, marker='X',
                    edgecolors='black', label='End 1')

    if start2 is not None:
        plt.scatter(tracked2[start2, 0], tracked2[start2, 1],
                    c='lime', s=250, marker='o',
                    edgecolors='black', label='Start 2')

    if end2 is not None:
        plt.scatter(tracked2[end2, 0], tracked2[end2, 1],
                    c='cyan', s=250, marker='X',
                    edgecolors='black', label='End 2')

    # ---- formatting ----
    plt.title("Throw Tracking with Temporal Color Gradient")
    plt.xlabel("X (pixels)")
    plt.ylabel("Y (pixels)")
    plt.gca().invert_yaxis()
    plt.grid(True, alpha=0.3)

    # colorbars (one per throw)
    cbar1 = plt.colorbar(sc1, fraction=0.02, pad=0.02)
    cbar1.set_label("Throw 1 time (start → end)")

    cbar2 = plt.colorbar(sc2, fraction=0.02, pad=0.08)
    cbar2.set_label("Throw 2 time (start → end)")

    plt.legend()
    plt.savefig('a1a2.png')
    plt.show()
    
