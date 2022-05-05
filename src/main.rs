use bevy::{
    pbr::wireframe::{Wireframe, WireframeConfig, WireframePlugin},
    prelude::*,
    render::{primitives::Aabb, render_resource::WgpuFeatures, settings::WgpuSettings},
    utils::Instant,
};
use bevy_inspector_egui::{Inspectable, InspectorPlugin};
use bevy_mod_picking::*;
use chunk::{Chunk, ChunkMesh};
use iters::Iter3d;
use marching_cube_tables::{EDGE_CONNECTION, EDGE_TABLE, TRIANGLE_TABLE};
use noise::{Fbm, MultiFractal, NoiseFn};

mod camera;
mod chunk;
mod iters;
mod marching_cube_tables;

const CHUNK_SIZE: usize = 12;
#[derive(Default)]
struct StartMarching;

#[derive(Component)]
struct Point(f32);

#[derive(Component)]
struct MarchCubeIndicator;

#[derive(Component)]
struct ChunkPoint;

#[derive(Inspectable)]
struct Data {
    #[inspectable(min = 0.0, max = 1.0, speed = 0.01)]
    isolevel: f32,
}

impl Default for Data {
    fn default() -> Self {
        Self { isolevel: 0.5 }
    }
}

#[derive(Inspectable)]
struct NoiseSettings {
    /// Total number of frequency octaves to generate the noise with.
    ///
    /// The number of octaves control the _amount of detail_ in the noise
    /// function. Adding more octaves increases the detail, with the drawback
    /// of increasing the calculation time.
    #[inspectable(min = 0, max = 32)]
    octaves: usize,

    /// The number of cycles per unit length that the noise function outputs.
    #[inspectable(min = 0.0, max = 5.0, speed = 0.1)]
    frequency: f64,

    /// A multiplier that determines how quickly the frequency increases for
    /// each successive octave in the noise function.
    ///
    /// The frequency of each successive octave is equal to the product of the
    /// previous octave's frequency and the lacunarity value.
    ///
    /// A lacunarity of 2.0 results in the frequency doubling every octave. For
    /// almost all cases, 2.0 is a good value to use.
    #[inspectable(min = 0.0, max = 5.0, speed = 0.1)]
    lacunarity: f64,

    /// A multiplier that determines how quickly the amplitudes diminish for
    /// each successive octave in the noise function.
    ///
    /// The amplitude of each successive octave is equal to the product of the
    /// previous octave's amplitude and the persistence value. Increasing the
    /// persistence produces "rougher" noise.
    #[inspectable(min = 0.05, max = 2.0, speed = 0.05)]
    persistence: f64,

    #[inspectable()]
    offset: UVec3,

    #[inspectable(min = 0.1, max = 1.5, speed = 0.01)]
    scale: f32,
}

impl Default for NoiseSettings {
    fn default() -> Self {
        Self {
            octaves: Fbm::DEFAULT_OCTAVE_COUNT,
            frequency: Fbm::DEFAULT_FREQUENCY,
            lacunarity: 0.2,
            persistence: Fbm::DEFAULT_PERSISTENCE,
            offset: UVec3::ZERO,
            scale: 1.0,
        }
    }
}

fn main() {
    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        #[cfg(target_arch = "wasm32")]
        canvas: Some(String::from("#bevy")),
        ..default()
    })
    .insert_resource(WgpuSettings {
        features: WgpuFeatures::POLYGON_MODE_LINE,
        ..default()
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(PickingPlugin)
    .add_plugin(InteractablePickingPlugin)
    .add_plugin(DebugCursorPickingPlugin)
    .add_plugin(InspectorPlugin::<Data>::new())
    .add_plugin(InspectorPlugin::<NoiseSettings>::new())
    .add_event::<StartMarching>()
    .add_startup_system(setup)
    .add_startup_system(setup_chunks)
    .add_system(select_event)
    .add_system(update_chunk)
    .add_system(camera::fly_camera)
    .add_system(start_march)
    .add_system(update_data)
    .add_system(update_noise_values)
    .add_system(update_points_color.after(update_noise_values));

    #[cfg(not(target_arch = "wasm32"))]
    {
        // app.add_plugin(WireframePlugin)
        // .insert_resource(WireframeConfig { global: false });
    }

    app.run();
}

fn setup(mut commands: Commands) {
    let chunk_size = CHUNK_SIZE as f32 + CHUNK_SIZE as f32 / 2.0;
    commands
        .spawn_bundle(PerspectiveCameraBundle {
            transform: Transform::from_xyz(chunk_size, chunk_size, chunk_size)
                .looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        })
        .insert_bundle(PickingCameraBundle::default())
        .insert(camera::FlyCam);

    commands.spawn_bundle(PointLightBundle {
        point_light: PointLight {
            intensity: chunk_size * 1000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_xyz(chunk_size, chunk_size, chunk_size),
        ..default()
    });
}

fn unlit_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        unlit: true,
        ..default()
    }
}

fn setup_chunks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let icosphere = meshes.add(Mesh::from(shape::Icosphere {
        radius: 0.05,
        ..default()
    }));

    let black = materials.add(unlit_material(Color::BLACK));

    for point in Chunk::new_iter_3d(CHUNK_SIZE as u32 + 1) {
        commands
            .spawn_bundle(PbrBundle {
                mesh: icosphere.clone(),
                material: black.clone(),
                transform: Transform::from_translation(point.as_vec3()),
                ..default()
            })
            .insert_bundle(PickableBundle::default())
            .insert(ChunkPoint);
    }

    spawn_chunk(
        &mut commands,
        &mut meshes,
        &mut materials,
        CHUNK_SIZE,
        Vec3::ZERO,
    );
}

fn spawn_chunk(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    size: usize,
    pos: Vec3,
) {
    let points = vec![0.0; (size + 1).pow(3)];
    let chunk_mesh = ChunkMesh::default();
    commands
        .spawn_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(chunk_mesh.clone())),
            material: materials.add(StandardMaterial {
                base_color: Color::rgba(1.0, 0.0, 0.0, 1.0),
                ..default()
            }),
            transform: Transform::from_translation(pos),
            ..default()
        })
        .insert(Chunk::new(points, size))
        .insert(Chunk::new_iter_3d(size as u32))
        .insert(chunk_mesh)
        .insert(Wireframe);
}

fn update_noise_values(mut chunks: Query<&mut Chunk>, noise_settings: Res<NoiseSettings>) {
    if !noise_settings.is_changed() {
        return;
    }

    let fbm = Fbm::new()
        .set_octaves(noise_settings.octaves)
        .set_persistence(noise_settings.persistence)
        .set_lacunarity(noise_settings.lacunarity)
        .set_frequency(noise_settings.frequency);

    for mut chunk in chunks.iter_mut() {
        for point in Chunk::new_iter_3d(chunk.size as u32) {
            let max = chunk.size as u32;
            let val = if point.x <= 0
                || point.x >= max
                || point.y <= 0
                || point.y >= max
                || point.z <= 0
                || point.z >= max
            {
                // make border empty
                0.0
            } else {
                let point = point + noise_settings.offset;
                let val = fbm.get([point.x as f64, point.y as f64, point.z as f64]);
                (val + 1.0) / 2.0
            };
            chunk.set(point.as_vec3(), val as f32 * noise_settings.scale);
        }
    }
}

fn select_event(
    mut events: EventReader<PickingEvent>,
    transforms: Query<&Transform>,
    mut chunks: Query<&mut Chunk>,
) {
    let mut chunk = chunks.single_mut();
    for event in events.iter() {
        if let PickingEvent::Clicked(entity) = event {
            if let Ok(&Transform { translation, .. }) = transforms.get(*entity) {
                let value = chunk.get(translation);
                chunk.set(translation, if value == 1.0 { 0.0 } else { 1.0 });
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

fn update_data(
    data: Res<Data>,
    noise_settings: Res<NoiseSettings>,
    mut start_marching_events: EventWriter<StartMarching>,
) {
    if data.is_changed() || noise_settings.is_changed() {
        start_marching_events.send_default();
    }
}

fn update_chunk(
    mut chunks: Query<(
        &Chunk,
        &mut Iter3d,
        &mut ChunkMesh,
        &Handle<Mesh>,
        Option<&mut Aabb>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut start_event: EventReader<StartMarching>,
    data: Res<Data>,
) {
    if start_event.iter().count() == 0 {
        return;
    }
    let start = Instant::now();

    let (chunk, mut chunk_iter, mut chunk_mesh, mesh_handle, chunk_aabb) = chunks.single_mut();
    chunk_iter.reset();
    chunk_mesh.triangles.clear();

    for pos in chunk_iter.into_iter() {
        let pos = pos.as_vec3();
        let mut grid_cell = GridCell::new(pos);
        for (i, v_pos) in grid_cell.vertex_position.iter().enumerate() {
            grid_cell.value[i] = chunk.get(*v_pos);
        }

        if let Some(triangles) = march_cube(&grid_cell, data.isolevel) {
            chunk_mesh.triangles.extend(triangles);
        }
    }
    let mesh = Mesh::from(chunk_mesh.clone());
    if let Some(mut chunk_aabb) = chunk_aabb {
        if let Some(aabb) = mesh.compute_aabb() {
            *chunk_aabb = aabb;
        }
    }
    meshes.set_untracked(mesh_handle, mesh);
    chunk_iter.reset();

    info!("Marching took {:?}", start.elapsed());
}

fn update_points_color(
    chunk: Query<&Chunk>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut q: Query<(&Transform, &mut Handle<StandardMaterial>, &mut Visibility), With<ChunkPoint>>,
    data: Res<Data>,
    noise_settings: Res<NoiseSettings>,
) {
    if !data.is_changed() && !noise_settings.is_changed() {
        return;
    }
    for chunk in chunk.iter() {
        info!("updating point color");
        for (transform, mut mat, mut visibility) in q.iter_mut() {
            let val = chunk.get(transform.translation);
            *mat = materials.add(unlit_material(Color::rgb(val, val, val)));
            visibility.is_visible = val >= data.isolevel || val == 0.0;
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
        GridCell {
            vertex_position: [
                pos + Vec3::new(0.0, 0.0, 0.0),
                pos + Vec3::new(1.0, 0.0, 0.0),
                pos + Vec3::new(1.0, 0.0, 1.0),
                pos + Vec3::new(0.0, 0.0, 1.0),
                pos + Vec3::new(0.0, 1.0, 0.0),
                pos + Vec3::new(1.0, 1.0, 0.0),
                pos + Vec3::new(1.0, 1.0, 1.0),
                pos + Vec3::new(0.0, 1.0, 1.0),
            ],
            value: [0.0; 8],
        }
    }
}
