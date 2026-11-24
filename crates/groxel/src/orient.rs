//! Faces of a voxel, and the 48 ways one cube can be laid onto another.

use crate::vec::{IVec3, Vec3};
use std::fmt;

/// One of the six faces of a voxel, named by its outward direction.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Face {
    /// +x
    PosX = 0,
    /// -x
    NegX = 1,
    /// +y
    PosY = 2,
    /// -y
    NegY = 3,
    /// +z
    PosZ = 4,
    /// -z
    NegZ = 5,
}

impl Face {
    /// All six, in index order.
    pub const ALL: [Face; 6] = [
        Face::PosX,
        Face::NegX,
        Face::PosY,
        Face::NegY,
        Face::PosZ,
        Face::NegZ,
    ];

    /// The face on `axis` (0, 1 or 2) pointing the positive or negative way.
    pub const fn new(axis: usize, positive: bool) -> Face {
        match (axis, positive) {
            (0, true) => Face::PosX,
            (0, false) => Face::NegX,
            (1, true) => Face::PosY,
            (1, false) => Face::NegY,
            (2, true) => Face::PosZ,
            _ => Face::NegZ,
        }
    }

    /// 0 for x, 1 for y, 2 for z.
    pub const fn axis(self) -> usize {
        (self as usize) >> 1
    }

    /// +1 or -1.
    pub const fn sign(self) -> i32 {
        if (self as u8) & 1 == 0 { 1 } else { -1 }
    }

    /// The face on the other side of the voxel.
    pub const fn opposite(self) -> Face {
        Face::new(self.axis(), self.sign() < 0)
    }

    /// The outward unit step through this face.
    pub const fn normal(self) -> IVec3 {
        let mut n = [0, 0, 0];
        n[self.axis()] = self.sign();
        IVec3(n)
    }

    /// The face a right-handed cross product of the two normals points to.
    /// `None` if the faces are on the same axis.
    pub fn cross(self, other: Face) -> Option<Face> {
        let (u, v) = (self.normal(), other.normal());
        let c = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        (0..3).find(|&i| c[i] != 0).map(|i| Face::new(i, c[i] > 0))
    }

    pub(crate) const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

const PERMS: [[u8; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

const fn encode(p: [u8; 3], s: [i8; 3]) -> u8 {
    let mut k = 0;
    while k < 6 {
        if PERMS[k][0] == p[0] && PERMS[k][1] == p[1] && PERMS[k][2] == p[2] {
            break;
        }
        k += 1;
    }
    let bits = (s[0] < 0) as u8 | ((s[1] < 0) as u8) << 1 | ((s[2] < 0) as u8) << 2;
    k as u8 * 8 + bits
}

const fn perm_of(i: u8) -> [u8; 3] {
    PERMS[(i / 8) as usize]
}

const fn signs_of(i: u8) -> [i8; 3] {
    let b = i % 8;
    [
        if b & 1 != 0 { -1 } else { 1 },
        if b & 2 != 0 { -1 } else { 1 },
        if b & 4 != 0 { -1 } else { 1 },
    ]
}

// THEN[a][b] is "a, then b". Built once, at compile time.
const THEN: [[u8; 48]; 48] = {
    let mut t = [[0u8; 48]; 48];
    let mut a = 0;
    while a < 48 {
        let (pa, sa) = (perm_of(a as u8), signs_of(a as u8));
        let mut b = 0;
        while b < 48 {
            let (pb, sb) = (perm_of(b as u8), signs_of(b as u8));
            let mut p = [0u8; 3];
            let mut s = [0i8; 3];
            let mut i = 0;
            while i < 3 {
                p[i] = pa[pb[i] as usize];
                s[i] = sb[i] * sa[pb[i] as usize];
                i += 1;
            }
            t[a][b] = encode(p, s);
            b += 1;
        }
        a += 1;
    }
    t
};

const INVERSE: [u8; 48] = {
    let mut t = [0u8; 48];
    let mut a = 0;
    while a < 48 {
        let (p, s) = (perm_of(a as u8), signs_of(a as u8));
        let mut q = [0u8; 3];
        let mut r = [0i8; 3];
        let mut i = 0;
        while i < 3 {
            q[p[i] as usize] = i as u8;
            r[p[i] as usize] = s[i];
            i += 1;
        }
        t[a] = encode(q, r);
        a += 1;
    }
    t
};

/// One of the 48 ways a cube maps onto a cube: a signed permutation of the
/// axes.
///
/// These are the only turns a seam can carry, because they are the only ones
/// that send the voxel lattice to itself. The value is an index into fixed
/// tables, so no other turn can be represented.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Orientation(u8);

impl Orientation {
    /// No turn.
    pub const IDENTITY: Orientation = Orientation(0);

    /// All 48: 24 rotations and 24 reflections.
    pub fn all() -> impl Iterator<Item = Orientation> {
        (0..48).map(Orientation)
    }

    /// This turn followed by `next`.
    pub const fn then(self, next: Orientation) -> Orientation {
        Orientation(THEN[self.0 as usize][next.0 as usize])
    }

    /// The turn that undoes this one.
    pub const fn inverse(self) -> Orientation {
        Orientation(INVERSE[self.0 as usize])
    }

    /// True for the 24 that flip handedness (mirrors).
    pub const fn is_reflection(self) -> bool {
        let (p, s) = (perm_of(self.0), signs_of(self.0));
        let inversions = (p[0] > p[1]) as i8 + (p[0] > p[2]) as i8 + (p[1] > p[2]) as i8;
        let parity = if inversions % 2 == 0 { 1 } else { -1 };
        parity * s[0] * s[1] * s[2] < 0
    }

    /// Turn a whole-voxel vector.
    pub const fn apply(self, v: IVec3) -> IVec3 {
        let (p, s) = (perm_of(self.0), signs_of(self.0));
        IVec3([
            v.0[p[0] as usize] * s[0] as i32,
            v.0[p[1] as usize] * s[1] as i32,
            v.0[p[2] as usize] * s[2] as i32,
        ])
    }

    /// Turn a direction.
    pub fn apply_f(self, v: Vec3) -> Vec3 {
        let (p, s) = (perm_of(self.0), signs_of(self.0));
        Vec3([
            v.0[p[0] as usize] * s[0] as f32,
            v.0[p[1] as usize] * s[1] as f32,
            v.0[p[2] as usize] * s[2] as f32,
        ])
    }

    /// Where a face ends up.
    pub const fn face(self, f: Face) -> Face {
        let j = self.axis_to(f.axis());
        Face::new(j, signs_of(self.0)[j] as i32 * f.sign() > 0)
    }

    /// Which axis the input axis `a` lands on.
    pub const fn axis_to(self, a: usize) -> usize {
        let p = perm_of(self.0);
        if p[0] as usize == a {
            0
        } else if p[1] as usize == a {
            1
        } else {
            2
        }
    }

    /// For output axis `j`: the input axis it reads, and the sign it applies.
    /// Used to shuffle per-axis state, such as a ray's stepping, across a seam.
    pub const fn source(self, j: usize) -> (usize, i32) {
        (perm_of(self.0)[j] as usize, signs_of(self.0)[j] as i32)
    }

    /// Build from where +x, +y and +z go. `None` unless the three land on
    /// three different axes.
    pub fn from_axes(x_to: Face, y_to: Face, z_to: Face) -> Option<Orientation> {
        let mut p = [9u8; 3];
        let mut s = [0i8; 3];
        for (k, f) in [x_to, y_to, z_to].into_iter().enumerate() {
            p[f.axis()] = k as u8;
            s[f.axis()] = f.sign() as i8;
        }
        if p.contains(&9) {
            None
        } else {
            Some(Orientation(encode(p, s)))
        }
    }

    /// The rotation that takes `a` to `a_to` and `b` to `b_to`. `None` if `a`
    /// and `b` share an axis, or `a_to` and `b_to` do.
    pub fn from_pairs(a: Face, a_to: Face, b: Face, b_to: Face) -> Option<Orientation> {
        let (c, c_to) = (a.cross(b)?, a_to.cross(b_to)?);
        let mut p = [0u8; 3];
        let mut s = [0i8; 3];
        for (f, t) in [(a, a_to), (b, b_to), (c, c_to)] {
            p[t.axis()] = f.axis() as u8;
            s[t.axis()] = (f.sign() * t.sign()) as i8;
        }
        Some(Orientation(encode(p, s)))
    }

    /// Right-handed rotation about +`axis` by `quarters` quarter turns.
    pub fn quarter_turn(axis: usize, quarters: i32) -> Orientation {
        let base = match axis {
            0 => Orientation::from_axes(Face::PosX, Face::PosZ, Face::NegY),
            1 => Orientation::from_axes(Face::NegZ, Face::PosY, Face::PosX),
            _ => Orientation::from_axes(Face::PosY, Face::NegX, Face::PosZ),
        }
        .expect("three distinct axes");
        (0..quarters.rem_euclid(4)).fold(Orientation::IDENTITY, |o, _| o.then(base))
    }

    /// Reflection across the plane perpendicular to `axis`.
    pub fn mirror(axis: usize) -> Orientation {
        let mut s = [1i8; 3];
        s[axis] = -1;
        Orientation(encode([0, 1, 2], s))
    }
}

impl Default for Orientation {
    fn default() -> Self {
        Orientation::IDENTITY
    }
}

impl fmt::Debug for Orientation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (p, s) = (perm_of(self.0), signs_of(self.0));
        let n = ["x", "y", "z"];
        write!(f, "Orientation(")?;
        for i in 0..3 {
            let sep = if i == 0 { "" } else { ", " };
            write!(
                f,
                "{sep}{}'={}{}",
                n[i],
                if s[i] < 0 { "-" } else { "" },
                n[p[i] as usize]
            )?;
        }
        write!(f, ")")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vec::ivec3;

    #[test]
    fn tables_agree_with_applying_in_order() {
        let v = ivec3(2, -5, 11);
        for a in Orientation::all() {
            assert_eq!(a.then(a.inverse()), Orientation::IDENTITY);
            assert_eq!(a.inverse().apply(a.apply(v)), v);
            for b in Orientation::all() {
                assert_eq!(a.then(b).apply(v), b.apply(a.apply(v)));
            }
        }
    }

    #[test]
    fn half_are_rotations() {
        assert_eq!(
            Orientation::all().filter(|o| !o.is_reflection()).count(),
            24
        );
        assert!(!Orientation::IDENTITY.is_reflection());
        assert!(Orientation::mirror(1).is_reflection());
    }

    #[test]
    fn quarter_turns() {
        let ry = Orientation::quarter_turn(1, 1);
        assert_eq!(ry.apply(ivec3(0, 0, 1)), ivec3(1, 0, 0));
        assert_eq!(ry.apply(ivec3(1, 0, 0)), ivec3(0, 0, -1));
        assert_eq!(Orientation::quarter_turn(1, 4), Orientation::IDENTITY);
        assert_eq!(
            Orientation::quarter_turn(0, -1),
            Orientation::quarter_turn(0, 3)
        );
        for axis in 0..3 {
            assert!(!Orientation::quarter_turn(axis, 1).is_reflection());
        }
    }

    #[test]
    fn faces_follow_vectors() {
        for o in Orientation::all() {
            for f in Face::ALL {
                assert_eq!(o.face(f).normal(), o.apply(f.normal()));
            }
        }
    }

    #[test]
    fn from_pairs_is_a_rotation_with_those_images() {
        let o = Orientation::from_pairs(Face::PosX, Face::PosY, Face::PosY, Face::NegX).unwrap();
        assert_eq!(o.face(Face::PosX), Face::PosY);
        assert_eq!(o.face(Face::PosY), Face::NegX);
        assert!(!o.is_reflection());
        assert!(Orientation::from_pairs(Face::PosX, Face::PosY, Face::NegX, Face::PosZ).is_none());
    }

    #[test]
    fn source_matches_apply() {
        let v = ivec3(3, 7, -2);
        for o in Orientation::all() {
            let out = o.apply(v);
            for j in 0..3 {
                let (src, sign) = o.source(j);
                assert_eq!(out[j], v[src] * sign);
            }
        }
    }
}
