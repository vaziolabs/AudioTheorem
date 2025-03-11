pub fn apply_filter(
    sample: f32,
    filter_type: &FilterType,
    cutoff: f32,
) -> f32 {
    if *filter_type == FilterType::None {
        return sample;
    }
    
    let cutoff = cutoff.clamp(0.01, 0.99);
    
    match filter_type {
        FilterType::LowPass => sample * cutoff.powf(0.5),
        FilterType::HighPass => sample * (1.0 - cutoff.powf(0.5)),
        FilterType::BandPass => sample * (1.0 - (cutoff - 0.5).abs() * 2.0),
        FilterType::None => sample,
    }
}
