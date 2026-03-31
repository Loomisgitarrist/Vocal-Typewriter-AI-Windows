#[allow(dead_code)]
pub trait Vad: Send + Sync {
    fn process(&mut self, audio: &[f32]) -> f32;
}