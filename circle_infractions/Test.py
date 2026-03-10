import random
import math
import matplotlib.pyplot as plt
# Event tuning parameters
# Same detection logic for every event.
# Only these numbers change per event (thresholds + timing).
EVENTS = {
    "Javelin (Arc Line)": {
        "touch_th": 0.10,
        "impact_th": 0.35,
        "gate_ms": 400,
        "debounce_ms": 70,
        "vib_hz": 6.0,
    },
    "Shot Put (Toe Board / Rim)": {
        "touch_th": 0.13,
        "impact_th": 0.45,
        "gate_ms": 500,
        "debounce_ms": 80,
        "vib_hz": 5.5,
    },
    "Discus (Rim)": {
        "touch_th": 0.12,
        "impact_th": 0.42,
        "gate_ms": 450,
        "debounce_ms": 75,
        "vib_hz": 6.2,
    },
    "Hammer (Rim)": {
        "touch_th": 0.14,
        "impact_th": 0.50,
        "gate_ms": 550,
        "debounce_ms": 90,
        "vib_hz": 5.0,
    }
}

# Fake sensor signal (simulation)

def simulate_signal(sample_hz, vib_hz, duration_s=6.0):
    """
    Generates a fake cap sensor trace:
      - baseline noise
      - one big impact spike + short vibration burst
      - a short brush contact (should not
       trigger)
      - a sustained contact (should trigger)
    """
    dt = 1.0 / sample_hz
    t = 0.0

    baseline = 1.0
    noise = 0.02

    impact_t = 1.8
    vib_len = 0.45

    brush_t0 = 2.9
    brush_t1 = 3.0

    touch_t0 = 3.6
    touch_t1 = 4.5

    times = []
    vals = []

    while t < duration_s:
        x = baseline + random.uniform(-noise, noise)

        # impact spike (foot strike / stomp)
        if abs(t - impact_t) < 0.03:
            x += 0.65

        # vibration after impact (5-6 Hz-ish)
        if impact_t < t < impact_t + vib_len:
            tau = t - impact_t
            damp = (0.85 ** (tau * 40))
            x += 0.22 * damp * math.sin(2 * math.pi * vib_hz * tau)

        # short brush (too short to count if debounce works)
        if brush_t0 <= t <= brush_t1:
            x += 0.16

        # sustained touch (should count)
        if touch_t0 <= t <= touch_t1:
            x += 0.22

        times.append(t)
        vals.append(x)
        t += dt

    return times, vals

# Detection logic (shared for all events)

def run_detector(times, raw_vals, touch_th, impact_th, gate_ms, debounce_ms, alpha=0.18, calib_s=1.5):
    """
    Simple pipeline:
      1) calibrate baseline
      2) low-pass filter
      3) impact gate (ignore triggers right after a big spike)
      4) threshold + debounce for stable "touch"
    """
    sample_hz = 1.0 / (times[1] - times[0])
    calib_n = int(calib_s * sample_hz)
    baseline = sum(raw_vals[:calib_n]) / calib_n

    filt = baseline
    gate_until = -1.0

    state = False
    candidate = False
    candidate_since = 0.0

    filtered_vals = []
    events = []  # list of (time, "INFRACTION"/"CLEAR")

    for i in range(len(times)):
        t = times[i]
        x = raw_vals[i]

        # low-pass filter
        filt = filt + alpha * (x - filt)
        filtered_vals.append(filt)

        delta = filt - baseline

        # slow baseline correction (only when quiet)
        if t > gate_until and abs(delta) < (touch_th * 0.5):
            baseline = baseline + 0.01 * (filt - baseline)

        # impact detection -> gate
        if abs(delta) > impact_th:
            gate_until = t + gate_ms / 1000.0

        # decide if we "should" be in touch state
        should_touch = False
        if t > gate_until:
            should_touch = (delta > touch_th)

        # debounce
        if should_touch != candidate:
            candidate = should_touch
            candidate_since = t

        if (t - candidate_since) >= (debounce_ms / 1000.0) and state != candidate:
            state = candidate
            events.append((t, "INFRACTION" if state else "CLEAR"))

    return filtered_vals, events


# Demo runner

def demo_event(event_name, params):
    sample_hz = 100.0
    times, raw_vals = simulate_signal(sample_hz, params["vib_hz"])

    filtered_vals, events = run_detector(
        times, raw_vals,
        touch_th=params["touch_th"],
        impact_th=params["impact_th"],
        gate_ms=params["gate_ms"],
        debounce_ms=params["debounce_ms"]
    )

    # Print event log
    print("\n=== " + event_name + " ===")
    for t_evt, name in events:
        print(f"{t_evt:5.2f}s  {name}")

    # Plot
    plt.figure()
    plt.title(event_name)
    plt.plot(times, raw_vals, label="raw")
    plt.plot(times, filtered_vals, label="filtered")

    # show approximate baseline and threshold line
    base_approx = sum(raw_vals[:150]) / 150
    plt.axhline(base_approx, linestyle="--", label="baseline (approx)")
    plt.axhline(base_approx + params["touch_th"], linestyle="--", label="touch threshold (approx)")

    # mark events
    for t_evt, name in events:
        plt.axvline(t_evt, linestyle=":", label=name)

    plt.xlabel("time (s)")
    plt.ylabel("sensor reading (fake units)")
    plt.legend(loc="upper right")
    plt.tight_layout()

def main():
    for name, params in EVENTS.items():
        demo_event(name, params)
    plt.show()

if __name__ == "__main__":
    main()