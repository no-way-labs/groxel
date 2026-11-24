//! Making seams, and stepping across them one voxel at a time.

use groxel::{
    Cell, Face, FaceRect, LatticeMap, Orientation, SeamError, SeamKind, World, ivec3, vec3,
};

mod common;

fn cell(room: groxel::RoomId, x: i32, y: i32, z: i32) -> Cell {
    Cell {
        room,
        at: ivec3(x, y, z),
    }
}

#[test]
fn wrap_joins_both_ends() {
    let mut w = World::new();
    let r = w.add_room("r", ivec3(4, 3, 5));
    w.wrap(r, 0).unwrap();
    assert_eq!(
        w.neighbour(cell(r, 3, 1, 2), Face::PosX).unwrap().0,
        cell(r, 0, 1, 2)
    );
    assert_eq!(
        w.neighbour(cell(r, 0, 2, 4), Face::NegX).unwrap().0,
        cell(r, 3, 2, 4)
    );
    assert!(w.neighbour(cell(r, 0, 0, 0), Face::NegY).is_none());
    // One seam each way, however large the face.
    assert_eq!(w.seams().len(), 2);
}

#[test]
fn a_glue_is_one_map_read_both_ways() {
    let n = 4;
    for turn in Orientation::all() {
        let cw = common::CutWorld::new(n, turn);
        let [there, back] = [cw.cut.seams()[0], cw.cut.seams()[1]];
        assert_eq!(there.map.then(back.map), LatticeMap::IDENTITY);
        assert_eq!(there.map.inverse(), back.map);

        for y in 0..n {
            for z in 0..n {
                let start = cell(cw.left, n - 1, y, z);
                let (over, seam) = cw.cut.neighbour(start, Face::PosX).unwrap();
                assert_eq!(over.room, cw.right);
                assert_eq!((over.room, over.at), cw.cell_in_cut(ivec3(n, y, z)));
                // Turn round: the way back is the way we came, re-read in
                // the far room's frame.
                let way_back = cw.cut.seam(seam.unwrap()).map.rot.face(Face::NegX);
                assert_eq!(cw.cut.neighbour(over, way_back).unwrap().0, start);
            }
        }
    }
}

#[test]
fn walk_agrees_with_the_uncut_world() {
    let n = 5;
    for turn in Orientation::all() {
        let cw = common::CutWorld::new(n, turn);
        let start = cell(cw.left, 3, 2, 2);
        for delta in [
            ivec3(4, 1, -2),
            ivec3(6, -2, 2),
            ivec3(2, 2, 2),
            ivec3(2, 0, 0),
        ] {
            let (end, picked_up) = cw.cut.walk(start, delta).unwrap();
            assert_eq!(
                (end.room, end.at),
                cw.cell_in_cut(start.at + delta),
                "turn {turn:?} delta {delta:?}"
            );
            assert_eq!(picked_up, turn);
        }
        // There and back again leaves no turn behind.
        let (there, _) = cw.cut.walk(start, ivec3(4, 0, 0)).unwrap();
        let home_dir = turn.apply(ivec3(-4, 0, 0));
        let (home, net) = cw.cut.walk(there, home_dir).unwrap();
        assert_eq!(home, start);
        assert_eq!(net, turn.inverse());
    }
}

#[test]
fn walk_goes_round_a_wrapped_room() {
    let mut w = World::new();
    let r = w.add_room("r", ivec3(5, 5, 5));
    w.wrap(r, 1).unwrap();
    let (end, turn) = w.walk(cell(r, 2, 4, 2), ivec3(1, 7, -1)).unwrap();
    assert_eq!(end, cell(r, 3, 1, 1));
    assert_eq!(turn, Orientation::IDENTITY);
}

#[test]
fn errors_say_what_is_wrong() {
    let mut w = World::new();
    let a = w.add_room("a", ivec3(4, 4, 4));
    let b = w.add_room("b", ivec3(4, 4, 4));
    let east = FaceRect {
        room: a,
        min: ivec3(3, 0, 0),
        max: ivec3(3, 3, 3),
        face: Face::PosX,
    };
    let west = FaceRect {
        room: b,
        min: ivec3(0, 0, 0),
        max: ivec3(0, 3, 3),
        face: Face::NegX,
    };

    // Both faces point the same way: you cannot walk out of one into the other.
    let also_east = FaceRect {
        face: Face::PosX,
        ..west
    };
    let err = w.glue(east, also_east, Orientation::IDENTITY).unwrap_err();
    assert_eq!(
        err,
        SeamError::FacesDoNotMeet {
            leave: Face::PosX,
            turned: Face::PosX,
            needed: Face::NegX
        }
    );
    assert!(
        err.to_string()
            .contains("Pick a turn that takes PosX to NegX")
    );

    // A doorway 4 x 2 cannot meet one that is 2 x 4 without a turn.
    let low = FaceRect {
        max: ivec3(3, 1, 3),
        ..east
    };
    let narrow = FaceRect {
        max: ivec3(0, 3, 1),
        ..west
    };
    let err = w.glue(low, narrow, Orientation::IDENTITY).unwrap_err();
    assert_eq!(
        err,
        SeamError::ShapesDiffer {
            turned: ivec3(1, 2, 4),
            far: ivec3(1, 4, 2)
        }
    );
    // With a quarter turn about the way through, it fits.
    w.glue(low, narrow, Orientation::quarter_turn(0, 1))
        .unwrap();

    // A face leads to one place.
    let err = w.glue(east, west, Orientation::IDENTITY).unwrap_err();
    assert!(
        matches!(
            err,
            SeamError::Overlap {
                face: Face::PosX,
                ..
            }
        ),
        "{err}"
    );

    let outside = FaceRect {
        room: a,
        min: ivec3(0, 0, 0),
        max: ivec3(9, 0, 0),
        face: Face::PosY,
    };
    assert!(matches!(
        w.link(outside, b, LatticeMap::IDENTITY, SeamKind::Open),
        Err(SeamError::OutsideRoom { .. })
    ));
    let top = FaceRect {
        room: a,
        min: ivec3(0, 3, 0),
        max: ivec3(0, 3, 0),
        face: Face::PosY,
    };
    assert!(matches!(
        w.link(top, b, LatticeMap::shift(ivec3(0, 5, 0)), SeamKind::Open),
        Err(SeamError::LandsOutside { .. })
    ));
    // A failed seam leaves nothing behind.
    assert_eq!(w.seams().len(), 2);
    assert!(
        w.seam_at(
            Cell {
                room: a,
                at: ivec3(0, 3, 0)
            },
            Face::PosY
        )
        .is_none()
    );
}

#[test]
fn glue_up_works_out_the_turn() {
    // A door in one room's wall that comes up through another room's floor.
    let mut w = World::new();
    let a = w.add_room("a", ivec3(32, 12, 8));
    let b = w.add_room("b", ivec3(16, 16, 8));
    let [there, _] = w
        .glue_up(
            FaceRect {
                room: a,
                min: ivec3(31, 1, 2),
                max: ivec3(31, 8, 5),
                face: Face::PosX,
            },
            FaceRect {
                room: b,
                min: ivec3(4, 0, 2),
                max: ivec3(11, 0, 5),
                face: Face::NegY,
            },
            Face::PosY,
            Face::NegX,
        )
        .unwrap();
    let (through, _) = w
        .neighbour(
            Cell {
                room: a,
                at: ivec3(31, 1, 2),
            },
            Face::PosX,
        )
        .unwrap();
    assert_eq!(through.room, b);
    // Walking east in A is going up in B.
    assert_eq!(w.seam(there).map.rot.face(Face::PosX), Face::PosY);
    assert!(matches!(
        w.glue_up(
            FaceRect {
                room: a,
                min: ivec3(0, 1, 2),
                max: ivec3(0, 8, 5),
                face: Face::NegX
            },
            FaceRect {
                room: b,
                min: ivec3(4, 15, 2),
                max: ivec3(11, 15, 5),
                face: Face::PosY
            },
            Face::PosX,
            Face::NegX,
        ),
        Err(SeamError::UpAlongTheWay)
    ));
}

#[test]
fn a_mirror_is_one_seam_back_to_itself() {
    let mut w = World::new();
    let r = w.add_room("r", ivec3(8, 3, 3));
    let glass = FaceRect {
        room: r,
        min: ivec3(7, 0, 0),
        max: ivec3(7, 2, 2),
        face: Face::PosX,
    };
    let id = w.mirror(glass).unwrap();
    let seam = *w.seam(id);
    assert_eq!(seam.kind, SeamKind::SightOnly);
    assert!(seam.map.rot.is_reflection());
    let here = Cell {
        room: r,
        at: ivec3(7, 1, 2),
    };
    assert_eq!(w.neighbour(here, Face::PosX).unwrap().0, here);
    // A point on the glass stays where it is; one in front lands behind.
    let on_glass = vec3(8.0, 1.25, 2.5);
    assert_eq!(seam.map.point(on_glass), on_glass);
    assert_eq!(seam.map.point(vec3(7.0, 1.25, 2.5)), vec3(9.0, 1.25, 2.5));
    assert_eq!(seam.map.then(seam.map), LatticeMap::IDENTITY);

    let thick = FaceRect {
        min: ivec3(6, 0, 0),
        ..glass
    };
    assert!(matches!(
        w.mirror(thick),
        Err(SeamError::MirrorNotFlat { .. })
    ));
}

#[test]
fn a_voxel_outside_its_room_has_no_neighbours() {
    let mut w = World::new();
    let r = w.add_room("r", ivec3(4, 4, 4));
    let outside = Cell {
        room: r,
        at: ivec3(9, 1, 1),
    };
    assert_eq!(w.neighbour(outside, Face::NegX), None);
    assert_eq!(w.walk(outside, ivec3(-1, 0, 0)), None);
    // Standing still is not a step, so it needs no neighbour.
    assert!(w.walk(outside, ivec3(0, 0, 0)).is_some());
}

#[test]
fn a_walk_for_solid_things_stops_at_a_mirror() {
    let mut w = World::new();
    let r = w.add_room("m", ivec3(8, 3, 3));
    w.mirror(FaceRect {
        room: r,
        min: ivec3(7, 0, 0),
        max: ivec3(7, 2, 2),
        face: Face::PosX,
    })
    .unwrap();
    let start = Cell {
        room: r,
        at: ivec3(5, 1, 1),
    };
    // Sight goes through the glass and comes back reflected.
    let (seen, turn) = w.walk(start, ivec3(4, 0, 0)).unwrap();
    assert_eq!(seen.at, ivec3(6, 1, 1));
    assert!(turn.is_reflection());
    // A solid thing gets as far as the glass and no further.
    assert_eq!(
        w.walk_open(start, ivec3(2, 0, 0)).map(|(c, _)| c.at),
        Some(ivec3(7, 1, 1))
    );
    assert_eq!(w.walk_open(start, ivec3(3, 0, 0)), None);
    // Open seams are no different for the two walks.
    w.wrap(r, 1).unwrap();
    assert_eq!(
        w.walk_open(start, ivec3(0, 5, 0)),
        w.walk(start, ivec3(0, 5, 0))
    );
}
