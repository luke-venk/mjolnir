// Determines whether or not our backend is using simulated throw data
// or using computer vision to process real footage from our cameras.
// This changes the function our `analyze-throw` route will use.
#[derive(Debug, Clone)]
pub enum ThrowSource {
    Simulated,
    Camera,
}
