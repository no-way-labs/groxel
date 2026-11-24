//! The Tower's cut is a screw dislocation. This test measures its Burgers
//! vector with a Burgers circuit, by the convention of Hirth and Lothe
//! (Theory of Dislocations, 1968, section 1-3): choose a sense for the
//! dislocation line; walk a circuit that is right-handed about that sense
//! and would close in a perfect lattice; the vector from its start to its
//! finish in the real lattice is the local Burgers vector b. A screw is
//! right-handed when b points along the line's sense and left-handed when it
//! points against it.

use groxel::{Cell, IVec3, Orientation, ivec3};
use groxel_app::Demo;

/// Walk the legs in order from `start` and return where they end.
fn circuit(demo: &Demo, start: Cell, legs: &[IVec3]) -> Cell {
    let mut at = start;
    for &leg in legs {
        let (next, turn) = demo
            .world
            .walk(at, leg)
            .expect("the circuit stays in the room");
        assert_eq!(turn, Orientation::IDENTITY);
        at = next;
    }
    at
}

fn tower(demo: &Demo) -> groxel::RoomId {
    let index = demo
        .world
        .rooms()
        .iter()
        .position(|room| room.name() == "tower")
        .expect("the demo has a tower");
    groxel::RoomId(index as u32)
}

#[test]
fn the_tower_is_a_left_handed_screw_dislocation_of_sixty_voxels() {
    let demo = Demo::new().expect("the worlds build");
    let room = tower(&demo);
    // East of the tower, one voxel north of the cut, at mid height.
    let start = Cell {
        room,
        at: ivec3(100, 200, 60),
    };

    // The line is vertical. Take its sense to be up, +y. A circuit that is
    // right-handed about +y goes from +x towards -z: south down the east
    // side, west along the south side, north up the west side, east along
    // the north side and south again to the start. Its legs sum to zero, so
    // it closes in a perfect lattice.
    let about_up = [
        ivec3(0, 0, -40),
        ivec3(-80, 0, 0),
        ivec3(0, 0, 80),
        ivec3(80, 0, 0),
        ivec3(0, 0, -40),
    ];
    assert_eq!(
        about_up.iter().fold(IVec3::ZERO, |sum, &leg| sum + leg),
        IVec3::ZERO
    );
    let finish = circuit(&demo, start, &about_up);
    let b = finish.at - start.at;
    // Sixty voxels against the line's sense: b . xi = -60, a left-handed
    // screw.
    assert_eq!(b, ivec3(0, -60, 0));

    // Reversing the sense of the line reverses b (Hirth and Lothe's axiom
    // 1-1), and b . xi is unchanged.
    let about_down: Vec<IVec3> = about_up.iter().rev().map(|&leg| -leg).collect();
    let finish = circuit(&demo, start, &about_down);
    assert_eq!(finish.at - start.at, ivec3(0, 60, 0));

    // A circuit that does not go round the line closes: out through the cut
    // and back through it, east of the tower.
    let beside = [
        ivec3(0, 0, -10),
        ivec3(10, 0, 0),
        ivec3(0, 0, 10),
        ivec3(-10, 0, 0),
    ];
    assert_eq!(circuit(&demo, start, &beside), start);
}
