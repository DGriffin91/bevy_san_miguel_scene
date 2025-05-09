use std::{f32::consts::PI, path::PathBuf, time::Instant};

mod camera_controller;
pub mod mipmap_generator;

use argh::FromArgs;

use bevy::{
    core_pipeline::{
        bloom::Bloom,
        experimental::taa::{TemporalAntiAliasPlugin, TemporalAntiAliasing},
    },
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::{CascadeShadowConfigBuilder, ScreenSpaceAmbientOcclusion, TransmittedShadowReceiver},
    prelude::*,
    render::view::{ColorGrading, NoFrustumCulling},
    window::{PresentMode, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use camera_controller::{CameraController, CameraControllerPlugin};
use mipmap_generator::{generate_mipmaps, MipmapGeneratorPlugin, MipmapGeneratorSettings};

use crate::convert::{change_gltf_to_use_ktx2, convert_images_to_ktx2};

mod convert;

#[derive(FromArgs, Resource, Clone)]
/// Config
pub struct Args {
    /// convert gltf to use ktx
    #[argh(switch)]
    convert: bool,

    /// disable bloom, AO, AA, shadows
    #[argh(switch)]
    minimal: bool,

    /// whether to disable frustum culling.
    #[argh(switch)]
    no_frustum_culling: bool,

    /// run at 720p (this scene is easily GPU limited)
    #[argh(switch)]
    p720: bool,

    /// compress textures (if they are not already, requires compress feature)
    #[argh(switch)]
    compress: bool,

    /// if low_quality_compression is set, only 0.5 byte/px formats will be used (BC1, BC4) unless the alpha channel is in use, then BC3 will be used.
    /// When low quality is set, compression is generally faster than CompressionSpeed::UltraFast and CompressionSpeed is ignored.
    #[argh(switch)]
    low_quality_compression: bool,

    /// compressed texture cache (requires compress feature)
    #[argh(switch)]
    cache: bool,
}

pub fn main() {
    let args: Args = argh::from_env();

    if args.convert {
        println!("This will take a few minutes");
        convert_images_to_ktx2();
        change_gltf_to_use_ktx2();
    }

    let mut app = App::new();

    app.insert_resource(args.clone())
        .insert_resource(ClearColor(Color::srgb(1.75, 1.8, 2.1)))
        .insert_resource(AmbientLight::NONE)
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::Continuous,
            unfocused_mode: UpdateMode::Continuous,
        })
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    present_mode: PresentMode::Immediate,
                    resolution: if args.p720 {
                        WindowResolution::new(1280.0, 720.0)
                    } else {
                        WindowResolution::new(1920.0, 1080.0)
                    }
                    .with_scale_factor_override(1.0),
                    ..default()
                }),
                ..default()
            }),
        )
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        // Generating mipmaps takes a minute
        .insert_resource(MipmapGeneratorSettings {
            anisotropic_filtering: 16,
            compression: Option::from(args.compress.then(Default::default)),
            compressed_image_data_cache_path: if args.cache {
                Some(PathBuf::from("compressed_texture_cache"))
            } else {
                None
            },
            low_quality: args.low_quality_compression,
            ..default()
        })
        .add_plugins((
            MipmapGeneratorPlugin,
            CameraControllerPlugin,
            TemporalAntiAliasPlugin,
        ))
        // Mipmap generation be skipped if ktx2 is used
        .add_systems(
            Update,
            (
                generate_mipmaps::<StandardMaterial>,
                proc_scene,
                input,
                benchmark,
            ),
        )
        .add_systems(Startup, setup);

    if args.no_frustum_culling {
        app.add_systems(Update, add_no_frustum_culling);
    }

    app.run();
}

#[derive(Component)]
pub struct PostProcScene;

#[derive(Component)]
pub struct GrifLight;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, args: Res<Args>) {
    println!("Loading models, generating mipmaps");

    commands.spawn((
        SceneRoot(asset_server.load("san-miguel/san-miguel.gltf#Scene0")),
        Transform::from_xyz(-18.0, 0.0, 0.0),
        PostProcScene,
    ));

    // Sun
    commands
        .spawn((
            Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, PI * -0.43, PI * -0.08, 0.0)),
            DirectionalLight {
                color: Color::srgb(0.95, 0.69268, 0.537758),
                illuminance: 2300000.0 * 0.2,
                shadows_enabled: !args.minimal,
                shadow_depth_bias: 0.04,
                shadow_normal_bias: 1.8,
                ..default()
            },
            CascadeShadowConfigBuilder {
                num_cascades: 4,
                maximum_distance: 30.0,
                ..default()
            }
            .build(),
        ))
        .insert(GrifLight);

    let point_spot_mult = 1000.0;
    // Sun Wall Refl
    commands.spawn((
        Transform::from_xyz(4.5, 4.0, 4.5).looking_at(Vec3::new(-999.0, 0.0, 0.0), Vec3::Y),
        SpotLight {
            range: 15.0,
            radius: 1.5,
            intensity: 250.0 * point_spot_mult,
            color: Color::srgb(1.75, 1.9, 1.9),
            shadows_enabled: false,
            inner_angle: PI * 0.4,
            outer_angle: PI * 0.5,
            ..default()
        },
        GrifLight,
    ));

    // Sun Ground Refl
    for t in [
        Transform::from_xyz(2.0, 0.5, 1.5),
        Transform::from_xyz(-1.5, 0.5, 1.5),
        Transform::from_xyz(-5.0, 0.5, 1.5),
    ] {
        commands.spawn((
            t.looking_at(Vec3::new(0.0, 999.0, 0.0), Vec3::X),
            SpotLight {
                range: 15.0,
                radius: 4.0,
                intensity: 1000.0 * point_spot_mult,
                color: Color::srgb(1.0, 0.85, 0.75),
                shadows_enabled: false,
                inner_angle: PI * 0.4,
                outer_angle: PI * 0.5,
                ..default()
            },
            GrifLight,
        ));
    }

    // Sun Table Refl
    for t in [
        Transform::from_xyz(2.95, 0.5, 3.15),
        Transform::from_xyz(-6.2, 0.5, 2.3),
    ] {
        commands.spawn((
            t.looking_at(Vec3::new(0.0, 999.0, 0.0), Vec3::X),
            SpotLight {
                range: 3.0,
                radius: 1.5,
                intensity: 150.0 * point_spot_mult,
                color: Color::srgb(1.0, 0.95, 0.9),
                shadows_enabled: false,
                inner_angle: PI * 0.4,
                outer_angle: PI * 0.5,
                ..default()
            },
            GrifLight,
        ));
    }

    // Camera
    let mut cam = commands.spawn((
        Camera3d::default(),
        Camera {
            hdr: true,
            ..default()
        },
        Transform::from_xyz(-10.5, 1.7, -1.0).looking_at(Vec3::new(0.0, 3.5, 0.0), Vec3::Y),
        Projection::Perspective(PerspectiveProjection {
            fov: std::f32::consts::PI / 3.0,
            ..default()
        }),
        ColorGrading {
            global: bevy::render::view::ColorGradingGlobal {
                exposure: -2.0,
                ..default()
            },
            ..default()
        },
        CameraController::default().print_controls(),
        Msaa::Off,
    ));

    if !args.minimal {
        cam.insert((
            Bloom {
                intensity: 0.05,
                ..default()
            },
            EnvironmentMapLight {
                diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
                specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
                intensity: 1000.0,
                ..default()
            },
            TemporalAntiAliasing::default(),
            ScreenSpaceAmbientOcclusion::default(),
        ));
    }
}

pub fn all_children<F: FnMut(Entity)>(
    children: &Children,
    children_query: &Query<&Children>,
    closure: &mut F,
) {
    for child in children {
        if let Ok(children) = children_query.get(*child) {
            all_children(children, children_query, closure);
        }
        closure(*child);
    }
}

#[allow(clippy::type_complexity)]
pub fn proc_scene(
    mut commands: Commands,
    post_proc_query: Query<Entity, With<PostProcScene>>,
    children_query: Query<&Children>,
    has_std_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    lights: Query<
        Entity,
        (
            Or<(With<PointLight>, With<DirectionalLight>, With<SpotLight>)>,
            Without<GrifLight>,
        ),
    >,
    cameras: Query<Entity, With<Camera>>,
) {
    for entity in post_proc_query.iter() {
        if let Ok(children) = children_query.get(entity) {
            all_children(children, &children_query, &mut |entity| {
                if let Ok(mat_h) = has_std_mat.get(entity) {
                    if let Some(mat) = materials.get_mut(mat_h) {
                        match mat.alpha_mode {
                            AlphaMode::Mask(_) => {
                                mat.diffuse_transmission = 0.6;
                                mat.double_sided = true;
                                mat.cull_mode = None;
                                mat.thickness = 0.2;
                                commands.entity(entity).insert(TransmittedShadowReceiver);
                            }
                            _ => (),
                        }
                    }
                }

                // Remove Default Lights
                if lights.get(entity).is_ok() {
                    commands.entity(entity).despawn();
                }

                // Remove Default Cameras
                if cameras.get(entity).is_ok() {
                    commands.entity(entity).despawn();
                }
            });
            commands.entity(entity).remove::<PostProcScene>();
        }
    }
}

const CAM_POS_1: Transform = Transform {
    translation: Vec3::new(-10.5, 1.7, -1.0),
    rotation: Quat::from_array([-0.05678932, 0.7372272, -0.062454797, -0.670351]),
    scale: Vec3::ONE,
};

const CAM_POS_2: Transform = Transform {
    translation: Vec3::new(4.8306146, 1.5906956, 12.70758),
    rotation: Quat::from_array([-0.02797842, -0.3963449, 0.012084955, -0.91759574]),
    scale: Vec3::ONE,
};

const CAM_POS_3: Transform = Transform {
    translation: Vec3::new(-14.211411, 6.807057, 1.6095632),
    rotation: Quat::from_array([0.0014981055, 0.71061265, 0.0015130794, -0.7035802]),
    scale: Vec3::ONE,
};

fn input(input: Res<ButtonInput<KeyCode>>, mut camera: Query<&mut Transform, With<Camera>>) {
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    if input.just_pressed(KeyCode::KeyI) {
        info!("{:?}", transform);
    }
    if input.just_pressed(KeyCode::Digit1) {
        *transform = CAM_POS_1
    }
    if input.just_pressed(KeyCode::Digit2) {
        *transform = CAM_POS_2
    }
    if input.just_pressed(KeyCode::Digit3) {
        *transform = CAM_POS_3
    }
}

fn benchmark(
    input: Res<ButtonInput<KeyCode>>,
    mut camera: Query<&mut Transform, With<Camera>>,
    materials: Res<Assets<StandardMaterial>>,
    meshes: Res<Assets<Mesh>>,
    has_std_mat: Query<&MeshMaterial3d<StandardMaterial>>,
    has_mesh: Query<&Mesh3d>,
    mut bench_started: Local<Option<Instant>>,
    mut bench_frame: Local<u32>,
    mut count_per_step: Local<u32>,
    time: Res<Time>,
) {
    if input.just_pressed(KeyCode::KeyB) && bench_started.is_none() {
        *bench_started = Some(Instant::now());
        *bench_frame = 0;
        // Try to render for around 2s or at least 30 frames per step
        *count_per_step = ((2.0 / time.delta_secs()) as u32).max(30);
        println!(
            "Starting Benchmark with {} frames per step",
            *count_per_step
        );
    }
    if bench_started.is_none() {
        return;
    }
    let Ok(mut transform) = camera.single_mut() else {
        return;
    };
    if *bench_frame == 0 {
        *transform = CAM_POS_1
    } else if *bench_frame == *count_per_step {
        *transform = CAM_POS_2
    } else if *bench_frame == *count_per_step * 2 {
        *transform = CAM_POS_3
    } else if *bench_frame == *count_per_step * 3 {
        let elapsed = bench_started.unwrap().elapsed().as_secs_f32();
        println!(
            "Benchmark avg cpu frame time: {:.2}ms",
            (elapsed / *bench_frame as f32) * 1000.0
        );
        println!(
            "Meshes: {}\nMesh Instances: {}\nMaterials: {}\nMaterial Instances: {}",
            meshes.len(),
            has_mesh.iter().len(),
            materials.len(),
            has_std_mat.iter().len(),
        );
        *bench_started = None;
        *bench_frame = 0;
        *transform = CAM_POS_1;
    }
    *bench_frame += 1;
}

pub fn add_no_frustum_culling(
    mut commands: Commands,
    convert_query: Query<
        Entity,
        (
            Without<NoFrustumCulling>,
            With<MeshMaterial3d<StandardMaterial>>,
        ),
    >,
) {
    for entity in convert_query.iter() {
        commands.entity(entity).insert(NoFrustumCulling);
    }
}
