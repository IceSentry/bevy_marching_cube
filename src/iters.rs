use bevy::prelude::*;

/// Linear iterator across a 3D coordinate space.
/// This iterator is inclusive of minimum and maximum coordinates.
#[derive(Component, Copy, Clone)]
pub struct Iter3d {
    track: UVec3,
    min: UVec3,
    max: UVec3,
}

impl Iter3d {
    #[must_use]
    pub fn new(min: UVec3, max: UVec3) -> Self {
        Self {
            track: min,
            min,
            max,
        }
    }

    pub fn reset(&mut self) {
        self.track = self.min;
    }
}

impl Iterator for Iter3d {
    type Item = UVec3;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.track;

        if self.track.z > self.max.z {
            return None;
        }

        if self.track.x >= self.max.x {
            self.track.y += 1;
            self.track.x = self.min.x;
        } else {
            self.track.x += 1;
            return Some(ret);
        }

        if self.track.y > self.max.y {
            self.track.z += 1;
            self.track.y = self.min.y;
        }

        Some(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::Iter3d;
    use bevy::math::UVec3;

    #[test]
    fn test() {
        let mut iter = Iter3d::new(UVec3::ZERO, UVec3::new(2, 2, 2));
        assert_eq!(iter.next(), Some(UVec3::ZERO));
    }
}
