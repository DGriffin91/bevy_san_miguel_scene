use std::{f32::consts::PI, time::Instant};

mod auto_instance;
mod camera_controller;
mod mipmap_generator;

use argh::FromArgs;
use auto_instance::{
    consolidate_material_instances, AutoInstanceMaterialPlugin, AutoInstancePlugin,
};
use bevy::{
    core_pipeline::{
        bloom::BloomSettings,
        experimental::taa::{TemporalAntiAliasBundle, TemporalAntiAliasPlugin},
    },
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    pbr::{
        CascadeShadowConfigBuilder, ScreenSpaceAmbientOcclusionBundle, TransmittedShadowReceiver,
    },
    prelude::*,
    render::view::{ColorGrading, NoFrustumCulling},
    window::{PresentMode, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use camera_controller::{CameraController, CameraControllerPlugin};
use mipmap_generator::{generate_mipmaps, MipmapGeneratorPlugin, MipmapGeneratorSettings};

use crate::{
    auto_instance::{AutoInstanceMaterialRecursive, AutoInstanceMeshRecursive},
    convert::{change_gltf_to_use_ktx2, convert_images_to_ktx2},
};

mod convert;

#[derive(FromArgs, Resource, Clone)]
/// Config
pub struct Args {
    /// convert gltf to use ktx
    #[argh(switch)]
    convert: bool,

    /// enable auto instancing for meshes/materials
    #[argh(switch)]
    instance: bool,

    /// disable bloom, AO, AA, shadows
    #[argh(switch)]
    minimal: bool,

    /// whether to disable frustum culling.
    #[argh(switch)]
    no_frustum_culling: bool,

    /// run at 720p (this scene is easily GPU limited)
    #[argh(switch)]
    p720: bool,
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
        .insert_resource(Msaa::Off)
        .insert_resource(ClearColor(Color::rgb(1.75, 1.8, 2.1)))
        .insert_resource(AmbientLight {
            color: Color::rgb(0.0, 0.0, 0.0),
            brightness: 0.0,
        })
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
        .add_plugins(FrameTimeDiagnosticsPlugin)
        // Generating mipmaps takes a minute
        .insert_resource(MipmapGeneratorSettings {
            anisotropic_filtering: 16,
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
                consolidate_material_instances::<StandardMaterial>,
                proc_scene,
                input,
                benchmark,
            ),
        )
        .add_systems(Startup, setup);

    if args.no_frustum_culling {
        app.add_systems(Update, add_no_frustum_culling);
    }
    if args.instance {
        app.add_plugins((
            AutoInstancePlugin,
            AutoInstanceMaterialPlugin::<StandardMaterial>::default(),
        ));
    }

    app.run();
}

#[derive(Component)]
pub struct PostProcScene;

#[derive(Component)]
pub struct GrifLight;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>, args: Res<Args>) {
    println!("Loading models, generating mipmaps");

    // San Miguel
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("san-miguel/san-miguel.gltf#Scene0"),
            transform: Transform::from_xyz(-18.0, 0.0, 0.0),
            ..default()
        },
        PostProcScene,
        AutoInstanceMaterialRecursive,
        AutoInstanceMeshRecursive,
    ));

    // Sun
    commands.spawn((
        DirectionalLightBundle {
            transform: Transform::from_rotation(Quat::from_euler(
                EulerRot::XYZ,
                PI * -0.43,
                PI * -0.08,
                0.0,
            )),
            directional_light: DirectionalLight {
                color: Color::rgb_linear(0.95, 0.69268, 0.537758),
                illuminance: 2300000.0 * 0.2,
                shadows_enabled: !args.minimal,
                shadow_depth_bias: 0.04,
                shadow_normal_bias: 1.8,
            },
            cascade_shadow_config: CascadeShadowConfigBuilder {
                num_cascades: 4,
                maximum_distance: 30.0,
                ..default()
            }
            .into(),
            ..default()
        },
        GrifLight,
    ));

    let point_spot_mult = 1000.0;
    // Sun Wall Refl
    commands.spawn((
        SpotLightBundle {
            transform: Transform::from_xyz(4.5, 4.0, 4.5)
                .looking_at(Vec3::new(-999.0, 0.0, 0.0), Vec3::Y),
            spot_light: SpotLight {
                range: 15.0,
                radius: 1.5,
                intensity: 250.0 * point_spot_mult,
                color: Color::rgb(1.75, 1.9, 1.9),
                shadows_enabled: false,
                inner_angle: PI * 0.4,
                outer_angle: PI * 0.5,
                ..default()
            },
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
            SpotLightBundle {
                transform: t.looking_at(Vec3::new(0.0, 999.0, 0.0), Vec3::X),
                spot_light: SpotLight {
                    range: 15.0,
                    radius: 4.0,
                    intensity: 1000.0 * point_spot_mult,
                    color: Color::rgb(1.0, 0.85, 0.75),
                    shadows_enabled: false,
                    inner_angle: PI * 0.4,
                    outer_angle: PI * 0.5,
                    ..default()
                },
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
            SpotLightBundle {
                transform: t.looking_at(Vec3::new(0.0, 999.0, 0.0), Vec3::X),
                spot_light: SpotLight {
                    range: 3.0,
                    radius: 1.5,
                    intensity: 150.0 * point_spot_mult,
                    color: Color::rgb(1.0, 0.95, 0.9),
                    shadows_enabled: false,
                    inner_angle: PI * 0.4,
                    outer_angle: PI * 0.5,
                    ..default()
                },
                ..default()
            },
            GrifLight,
        ));
    }

    // Camera
    let mut cam = commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            transform: Transform::from_xyz(-10.5, 1.7, -1.0)
                .looking_at(Vec3::new(0.0, 3.5, 0.0), Vec3::Y),
            projection: Projection::Perspective(PerspectiveProjection {
                fov: std::f32::consts::PI / 3.0,
                ..default()
            }),
            color_grading: ColorGrading {
                #[cfg(not(feature = "bevy_main"))]
                exposure: -2.0,
                #[cfg(feature = "bevy_main")]
                global: bevy::render::view::ColorGradingGlobal {
                    exposure: -2.0,
                    ..default()
                },
                ..default()
            },
            ..default()
        },
        CameraController::default().print_controls(),
    ));

    if !args.minimal {
        cam.insert((
            BloomSettings {
                intensity: 0.05,
                ..default()
            },
            EnvironmentMapLight {
                diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
                specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
                intensity: 1000.0,
            },
            TemporalAntiAliasBundle::default(),
        ))
        .insert(ScreenSpaceAmbientOcclusionBundle::default());
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
    materials_query: Query<Entity, With<PostProcScene>>,
    children_query: Query<&Children>,
    has_std_mat: Query<&Handle<StandardMaterial>>,
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
    for entity in materials_query.iter() {
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
                    commands.entity(entity).despawn_recursive();
                }

                // Remove Default Cameras
                if cameras.get(entity).is_ok() {
                    commands.entity(entity).despawn_recursive();
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
    let Ok(mut transform) = camera.get_single_mut() else {
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
    mut bench_started: Local<Option<Instant>>,
    mut bench_frame: Local<u32>,
    mut count_per_step: Local<u32>,
    time: Res<Time>,
) {
    if input.just_pressed(KeyCode::KeyB) && bench_started.is_none() {
        *bench_started = Some(Instant::now());
        *bench_frame = 0;
        // Try to render for around 2s or at least 30 frames per step
        *count_per_step = ((2.0 / time.delta_seconds()) as u32).max(30);
        println!(
            "Starting Benchmark with {} frames per step",
            *count_per_step
        );
    }
    if bench_started.is_none() {
        return;
    }
    let Ok(mut transform) = camera.get_single_mut() else {
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
        *bench_started = None;
        *bench_frame = 0;
        *transform = CAM_POS_1;
    }
    *bench_frame += 1;
}

pub fn add_no_frustum_culling(
    mut commands: Commands,
    convert_query: Query<Entity, (Without<NoFrustumCulling>, With<Handle<StandardMaterial>>)>,
) {
    for entity in convert_query.iter() {
        commands.entity(entity).insert(NoFrustumCulling);
    }
}
