use super::coordinates::unit_byte;

pub(super) fn spectral_color(progress: f32, byte: u8) -> [u8; 4] {
    let (start, end, amount) = if progress < 0.38 {
        ([22.0, 62.0, 255.0], [44.0, 216.0, 255.0], progress / 0.38)
    } else if progress < 0.7 {
        (
            [44.0, 216.0, 255.0],
            [232.0, 250.0, 255.0],
            (progress - 0.38) / 0.32,
        )
    } else {
        (
            [232.0, 250.0, 255.0],
            [255.0, 164.0, 38.0],
            (progress - 0.7) / 0.3,
        )
    };
    let brightness = unit_byte(byte).mul_add(0.42, 0.58);
    [
        color_channel(start[0], end[0], amount, brightness),
        color_channel(start[1], end[1], amount, brightness),
        color_channel(start[2], end[2], amount, brightness),
        150,
    ]
}

pub(super) fn entropy_color(entropy: f32, byte: u8) -> [u8; 4] {
    let (start, end, amount) = if entropy < 0.58 {
        ([18.0, 38.0, 132.0], [42.0, 224.0, 255.0], entropy / 0.58)
    } else {
        (
            [42.0, 224.0, 255.0],
            [255.0, 156.0, 32.0],
            (entropy - 0.58) / 0.42,
        )
    };
    let brightness = unit_byte(byte).mul_add(0.28, 0.72);
    [
        color_channel(start[0], end[0], amount, brightness),
        color_channel(start[1], end[1], amount, brightness),
        color_channel(start[2], end[2], amount, brightness),
        170,
    ]
}

pub(super) fn value_color(value: u8) -> [u8; 4] {
    let amount = unit_byte(value);
    [
        color_channel(34.0, 247.0, amount, 1.0),
        color_channel(190.0, 232.0, amount, 1.0),
        color_channel(220.0, 74.0, amount, 1.0),
        210,
    ]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn mix_color(first: [u8; 4], second: [u8; 4], amount: f32) -> [u8; 4] {
    let channel = |index: usize| {
        (f32::from(second[index]) - f32::from(first[index]))
            .mul_add(amount, f32::from(first[index]))
            .clamp(0.0, 255.0)
            .round() as u8
    };
    [channel(0), channel(1), channel(2), channel(3)]
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn color_channel(start: f32, end: f32, amount: f32, brightness: f32) -> u8 {
    ((end - start).mul_add(amount.clamp(0.0, 1.0), start) * brightness)
        .clamp(0.0, 255.0)
        .round() as u8
}
