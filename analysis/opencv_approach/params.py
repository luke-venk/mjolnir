# 1k
# (1024, 750)

# (2048, 1500),

# 4k:
# (4096, 3000),

p = '4k'
SCALE = 1.0
DISPLAY_WIDTH, DISPLAY_HEIGHT  = 1920, 1080
PROCESS_HEIGHT, PROCESS_WIDTH = 960, 540


if p == '4k':
    SCALE = 4.0
    MOG2_VAR_THRESHOLD  = 80    # lower = more sensitive; raise if noisy background
elif p == '2k':
    SCALE = 2.0


AREA_SCALE = SCALE ** 2

def s(x): return int(x * SCALE)
def a(x): return int(x * AREA_SCALE)

DISPLAY_WIDTH, DISPLAY_HEIGHT = DISPLAY_WIDTH, DISPLAY_HEIGHT

PROCESS_WIDTH, PROCESS_HEIGHT = s(PROCESS_HEIGHT), s(PROCESS_HEIGHT)


# Background subtractor
MOG2_HISTORY        = 300   # frames to build background model
MOG2_VAR_THRESHOLD  = 60    # lower = more sensitive; raise if noisy background
MOG2_DETECT_SHADOWS = False

MORPH_OPEN_KERNEL  = s(3)   # removes small noise blobs
MORPH_CLOSE_KERNEL = s(40)  # fills holes inside the shot put blob

# ROI tracking after initialization
ROI_SIZE = s(50)  # pixels in process-space (width and height of ROI)
ROI_PADDING = s(20)  # extra padding around predicted position
MIN_ROI_SIZE = s(100)  # minimum ROI size when not initialized

# Consistency check parameters
CONSISTENCY_WINDOW = 3  # number of frames to check for consistency
MAX_DISTANCE_VARIATION = 5  # maximum allowed variation in distances (pixels)
MIN_CONSISTENT_DETECTIONS = 4  # REDUCED from 5 to 2 - faster ROI activation

MIN_AREA            = a(25)    # px^2 — ignore tiny noise
MAX_AREA            = a(150)   # px^2 — ignore huge regions
MAX_PERIMETER       = s(70)    # px — ignore very large contours (athlete body)
MIN_CIRCULARITY     = 0.72#68   # 1.0 = perfect circle; lower catches slight blur61
MAX_ASPECT_RATIO    = 1.7    # width/height of bounding rect; rejects lines

MAX_MISSED_FRAMES   = 8     # frames without detection before tracker resets
TRAIL_LENGTH        = 60    # how many past positions to draw as trail
