//! Two small vector types: whole voxels and points between them.
//!
//! The crate has no dependencies, so these are its own. Both index by axis
//! (`v[0]` is x), because most of the engine works one axis at a time.

use std::ops::{Add, AddAssign, Index, IndexMut, Mul, Neg, Sub};

/// A voxel coordinate, or a whole-voxel offset.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct IVec3(pub [i32; 3]);

/// A point or direction in a room's frame, in voxels.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Vec3(pub [f32; 3]);

/// Shorthand for [`IVec3`].
pub const fn ivec3(x: i32, y: i32, z: i32) -> IVec3 {
    IVec3([x, y, z])
}

/// Shorthand for [`Vec3`].
pub const fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3([x, y, z])
}

impl IVec3 {
    /// The origin.
    pub const ZERO: IVec3 = IVec3([0, 0, 0]);

    /// The middle of this voxel.
    pub fn centre(self) -> Vec3 {
        Vec3([
            self.0[0] as f32 + 0.5,
            self.0[1] as f32 + 0.5,
            self.0[2] as f32 + 0.5,
        ])
    }

    /// This voxel's lowest corner as a point.
    pub fn corner(self) -> Vec3 {
        Vec3([self.0[0] as f32, self.0[1] as f32, self.0[2] as f32])
    }

    /// Component-wise minimum.
    pub fn min(self, o: IVec3) -> IVec3 {
        IVec3([
            self.0[0].min(o.0[0]),
            self.0[1].min(o.0[1]),
            self.0[2].min(o.0[2]),
        ])
    }

    /// Component-wise maximum.
    pub fn max(self, o: IVec3) -> IVec3 {
        IVec3([
            self.0[0].max(o.0[0]),
            self.0[1].max(o.0[1]),
            self.0[2].max(o.0[2]),
        ])
    }

    /// Component-wise absolute value.
    pub fn abs(self) -> IVec3 {
        IVec3([self.0[0].abs(), self.0[1].abs(), self.0[2].abs()])
    }
}

impl Vec3 {
    /// The origin.
    pub const ZERO: Vec3 = Vec3([0.0, 0.0, 0.0]);

    /// The voxel this point is in.
    pub fn floor(self) -> IVec3 {
        IVec3([
            self.0[0].floor() as i32,
            self.0[1].floor() as i32,
            self.0[2].floor() as i32,
        ])
    }

    /// Dot product.
    pub fn dot(self, o: Vec3) -> f32 {
        self.0[0] * o.0[0] + self.0[1] * o.0[1] + self.0[2] * o.0[2]
    }

    /// Cross product.
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3([
            self.0[1] * o.0[2] - self.0[2] * o.0[1],
            self.0[2] * o.0[0] - self.0[0] * o.0[2],
            self.0[0] * o.0[1] - self.0[1] * o.0[0],
        ])
    }

    /// Length.
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// The same direction at length one. Zero stays zero.
    pub fn normalized(self) -> Vec3 {
        let len = self.length();
        if len > 0.0 { self * (1.0 / len) } else { self }
    }
}

macro_rules! vec_ops {
    ($t:ident, $s:ty) => {
        impl Index<usize> for $t {
            type Output = $s;
            fn index(&self, i: usize) -> &$s {
                &self.0[i]
            }
        }
        impl IndexMut<usize> for $t {
            fn index_mut(&mut self, i: usize) -> &mut $s {
                &mut self.0[i]
            }
        }
        impl Add for $t {
            type Output = $t;
            fn add(self, o: $t) -> $t {
                $t([self.0[0] + o.0[0], self.0[1] + o.0[1], self.0[2] + o.0[2]])
            }
        }
        impl AddAssign for $t {
            fn add_assign(&mut self, o: $t) {
                *self = *self + o;
            }
        }
        impl Sub for $t {
            type Output = $t;
            fn sub(self, o: $t) -> $t {
                $t([self.0[0] - o.0[0], self.0[1] - o.0[1], self.0[2] - o.0[2]])
            }
        }
        impl Neg for $t {
            type Output = $t;
            fn neg(self) -> $t {
                $t([-self.0[0], -self.0[1], -self.0[2]])
            }
        }
        impl Mul<$s> for $t {
            type Output = $t;
            fn mul(self, k: $s) -> $t {
                $t([self.0[0] * k, self.0[1] * k, self.0[2] * k])
            }
        }
    };
}
vec_ops!(IVec3, i32);
vec_ops!(Vec3, f32);
