use bevy::{
    pbr::wireframe::{Wireframe, WireframeConfig, WireframePlugin},
    prelude::*,
};
use bevy_mod_picking::*;
use chunk::{Chunk, ChunkMesh};
use iters::Iter3d;
use marching_cube_tables::{EDGE_CONNECTION, EDGE_TABLE, TRIANGLE_TABLE};
use noise::{NoiseFn, OpenSimplex};

mod camera;
mod chunk;
mod iters;
mod marching_cube_tables;

const CHUNK_SIZE: usize = 10;
const TIMER_DURATION: f32 = 0.00001;
const ISOLEVEL: f32 = 0.55;

#[derive(Default)]
struct UpdatePointsMesh;

#[derive(Default)]
struct StartMarching;

#[derive(Component)]
struct Point(f32);

struct MarchTimer(Timer);

#[derive(Component)]
struct MarchCubeIndicator;

#[derive(Component)]
struct ChunkPoint;

fn main() {
    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        #[cfg(target_arch = "wasm32")]
        canvas: Some(String::from("#bevy")),
        ..default()
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(PickingPlugin)
    .add_plugin(InteractablePickingPlugin)
    .add_plugin(DebugCursorPickingPlugin) // <- Adds the green debug cursor.
    .add_event::<UpdatePointsMesh>()
    .add_event::<StartMarching>()
    .add_startup_system(setup)
    .add_startup_system(setup_points)
    .add_system(select_event)
    .add_system(update_chunk.after(select_event))
    .add_system(camera::fly_camera)
    .add_system(start_march)
    .add_system(update_points_color);

    #[cfg(not(target_arch = "wasm32"))]
    {
        app.add_plugin(WireframePlugin)
            .insert_resource(WireframeConfig { global: false });
    }

    app.run();
}

fn setup(mut commands: Commands) {
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(7.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert_bundle(PickingCameraBundle::default())
        .insert(camera::FlyCam);

    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: 1500.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 10.0, -5.0),
        ..default()
    });
}

fn setup_points(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let icosphere = meshes.add(Mesh::from(shape::Icosphere {
        radius: 0.05,
        ..default()
    }));

    let black = materials.add(Color::BLACK.into());

    let simplex = OpenSimplex::new();

    // TODO spawn multiple chunks
    let mut points = Vec::new();
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let max = CHUNK_SIZE - 1;
                let val = if x == 0 || x == max || y == 0 || y == max || z == 0 || z == max {
                    // make border empty
                    0.0
                } else {
                    let val = simplex.get([x as f64, y as f64, z as f64]);
                    (val + 1.0) / 2.0
                };
                points.push(val as f32);
                commands
                    .spawn_bundle(PbrBundle {
                        mesh: icosphere.clone(),
                        material: black.clone(),
                        transform: Transform::from_xyz(x as f32, y as f32, z as f32),
                        ..default()
                    })
                    .insert_bundle(PickableBundle::default())
                    .insert(ChunkPoint);
            }
        }
    }

    let chunk_mesh = ChunkMesh::default();

    commands
        .spawn()
        .insert(Chunk::new(points, CHUNK_SIZE))
        .insert(Chunk::new_iter(CHUNK_SIZE as u32))
        .insert(chunk_mesh.clone())
        .insert_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(chunk_mesh)),
            material: materials.add(StandardMaterial {
                base_color: Color::rgba(1.0, 0.0, 0.0, 1.0),
                ..default()
            }),
            ..default()
        })
        .insert(Wireframe);

    commands.insert_resource(MarchTimer(Timer::from_seconds(TIMER_DURATION, true)));

    // TODO insert it as child of chunk?
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            material: materials.add(StandardMaterial {
                base_color: Color::rgba(0.0, 0.0, 1.0, 0.25),
                alpha_mode: AlphaMode::Blend,
                ..default()
            }),
            visibility: Visibility { is_visible: false },
            ..default()
        })
        .insert(MarchCubeIndicator);
}

fn select_event(
    mut events: EventReader<PickingEvent>,
    transforms: Query<&Transform>,
    mut chunks: Query<&mut Chunk>,
    mut update_events: EventWriter<UpdatePointsMesh>,
) {
    let mut chunk = chunks.single_mut();
    for event in events.iter() {
        if let PickingEvent::Clicked(entity) = event {
            if let Ok(&Transform { translation, .. }) = transforms.get(*entity) {
                let value = chunk.get(translation);
                chunk.set(translation, if value == 1.0 { 0.0 } else { 1.0 });
                update_events.send_default();
            }
        }
    }
}

fn start_march(
    keyboard_input: Res<Input<KeyCode>>,
    mut start_marching_events: EventWriter<StartMarching>,
) {
    if keyboard_input.just_pressed(KeyCode::R) {
        start_marching_events.send_default();
    }
}

fn update_chunk(
    time: Res<Time>,
    mut chunks: Query<(&Chunk, &mut Iter3d, &mut ChunkMesh, &mut Handle<Mesh>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut timer: ResMut<MarchTimer>,
    mut start_event: EventReader<StartMarching>,
    mut indicator: Query<(&mut Transform, &mut Visibility), With<MarchCubeIndicator>>,
    mut marching: Local<bool>,
) {
    let (mut indicator_transform, mut indicator_visibility) = indicator.single_mut();

    // TODO loop on multiple chunks at the same time
    let (chunk, mut chunk_iter, mut chunk_mesh, mut mesh_handle) = chunks.single_mut();

    if start_event.iter().count() > 0 {
        info!("Start marching");
        *marching = true;
        indicator_visibility.is_visible = true;
        indicator_transform.translation = Vec3::new(0.5, 0.5, 0.5);
        chunk_iter.reset();
        chunk_mesh.triangles.clear();
    }

    if !*marching {
        return;
    }

    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    match chunk_iter.next() {
        Some(pos) => {
            let pos = pos.as_vec3();
            indicator_transform.translation = pos + Vec3::new(0.5, 0.5, 0.5);

            let mut grid_cell = GridCell::new(pos);
            for (i, v_pos) in grid_cell.vertex_position.iter().enumerate() {
                grid_cell.value[i] = chunk.get(*v_pos);
            }

            if let Some(triangles) = march_cube(&grid_cell, ISOLEVEL) {
                chunk_mesh.triangles.extend(triangles);
                *mesh_handle = meshes.add(Mesh::from(chunk_mesh.clone()));
            }
        }
        None => {
            info!("Marching is over");
            *marching = false;
            indicator_visibility.is_visible = false;
            chunk_iter.reset();
        }
    }
}

fn update_points_color(
    chunk: Query<&Chunk, Changed<Chunk>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&Transform, &mut Handle<StandardMaterial>, &mut Visibility), With<ChunkPoint>>,
) {
    for chunk in chunk.iter() {
        for (transform, mut mat, mut visibility) in q.iter_mut() {
            let val = chunk.get(transform.translation);
            *mat = materials.add(Color::rgb(val, val, val).into());
            visibility.is_visible = val >= ISOLEVEL || val == 0.0;
        }
    }
}

/// March a single cube
//     4--------5     *-----4------*
//    /|       /|    /|           /|
//   / |      / |   7 |          5 |
//  /  |     /  |  /  8         /  9
// 7--------6   | *------6-----*   |
// |   |    |   | |   |        |   |
// |   0----|---1 |   *-----0--|---*
// |  /     |  /  11 /         10 /
// | /      | /   | 3          | 1
// |/       |/    |/           |/
// 3--------2     *-----2------*
fn march_cube(grid: &GridCell, isolevel: f32) -> Option<Vec<Triangle>> {
    let mut cube_index: usize = 0;
    for i in 0..8 {
        if grid.value[i] < isolevel {
            cube_index |= 1 << i;
        };
    }

    let edge = EDGE_TABLE[cube_index];
    if edge == 0 {
        return None;
    }

    let mut vertices = [Vec3::ZERO; 12];
    for i in 0..12 {
        if edge & 1 << i != 0 {
            let [u, v] = EDGE_CONNECTION[i];
            vertices[i] = vertex_interp(
                isolevel,
                grid.vertex_position[u],
                grid.vertex_position[v],
                grid.value[u],
                grid.value[v],
            );
        }
    }

    let mut triangles = Vec::new();
    let triangulation = TRIANGLE_TABLE[cube_index];
    for i in (0..16).step_by(3) {
        if triangulation[i] < 0 {
            break;
        }
        triangles.push([
            vertices[triangulation[i + 2] as usize],
            vertices[triangulation[i + 1] as usize],
            vertices[triangulation[i] as usize],
        ]);
    }
    Some(triangles)
}

// Interpolate between 2 vertices proportional to isolevel
fn vertex_interp(isolevel: f32, p1: Vec3, p2: Vec3, valp1: f32, valp2: f32) -> Vec3 {
    if (isolevel - valp1).abs() < 0.00001 {
        return p1;
    }
    if (isolevel - valp2).abs() < 0.00001 {
        return p2;
    }
    if (valp1 - valp2).abs() < 0.00001 {
        return p1;
    }
    let mu = (isolevel - valp1) / (valp2 - valp1);
    p1 + mu * (p2 - p1)

    // always pick the mid-point
    // (p1 + p2) / 2.0
}

type Triangle = [Vec3; 3];

#[derive(Clone, Copy)]
struct GridCell {
    vertex_position: [Vec3; 8],
    value: [f32; 8],
}

impl GridCell {
    fn new(pos: Vec3) -> Self {
        let mut vertex_position = [Vec3::ZERO; 8];
        vertex_position[0] = pos + Vec3::new(0.0, 0.0, 0.0);
        vertex_position[1] = pos + Vec3::new(1.0, 0.0, 0.0);
        vertex_position[2] = pos + Vec3::new(1.0, 0.0, 1.0);
        vertex_position[3] = pos + Vec3::new(0.0, 0.0, 1.0);
        vertex_position[4] = pos + Vec3::new(0.0, 1.0, 0.0);
        vertex_position[5] = pos + Vec3::new(1.0, 1.0, 0.0);
        vertex_position[6] = pos + Vec3::new(1.0, 1.0, 1.0);
        vertex_position[7] = pos + Vec3::new(0.0, 1.0, 1.0);
        GridCell {
            vertex_position,
            value: [0.0; 8],
        }
    }
}
