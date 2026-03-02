// We currently are only using 2 ground cameras.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraId {
    Camera1,
    Camera2,
}
