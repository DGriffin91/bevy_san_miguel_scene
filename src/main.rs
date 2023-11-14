use std::f32::consts::PI;

mod auto_instance;
mod camera_controller;
mod mipmap_generator;

use auto_instance::{consolidate_material_instances, AutoInstancePlugin};
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
    render::view::ColorGrading,
};
use camera_controller::{CameraController, CameraControllerPlugin};
use mipmap_generator::{generate_mipmaps, MipmapGeneratorPlugin, MipmapGeneratorSettings};

use crate::convert::{change_gltf_to_use_ktx2, convert_images_to_ktx2};

mod convert;

pub fn main() {
    let args = &mut std::env::args();
    args.next();
    if let Some(arg) = &args.next() {
        if arg == "--convert" {
            println!("This will take a few minutes");
            convert_images_to_ktx2();
            change_gltf_to_use_ktx2();
        }
    }

    let mut app = App::new();

    app.insert_resource(Msaa::Off)
        .insert_resource(ClearColor(Color::rgb(1.75, 1.8, 2.1)))
        .insert_resource(AmbientLight {
            color: Color::rgb(0.0, 0.0, 0.0),
            brightness: 0.0,
        })
        .add_plugins(DefaultPlugins)
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        // Generating mipmaps takes a minute
        .insert_resource(MipmapGeneratorSettings {
            anisotropic_filtering: 16,
            ..default()
        })
        .add_plugins((
            AutoInstancePlugin,
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
            ),
        )
        .add_systems(Startup, setup);

    app.run();
}

#[derive(Component)]
pub struct PostProcScene;

#[derive(Component)]
pub struct GrifLight;

pub fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    println!("Loading models, generating mipmaps");

    // San Miguel
    commands.spawn((
        SceneBundle {
            scene: asset_server.load("san-miguel/san-miguel.gltf#Scene0"),
            transform: Transform::from_xyz(-18.0, 0.0, 0.0),
            ..default()
        },
        PostProcScene,
        //AutoInstanceMaterialRecursive, // This is maybe ok
        //AutoInstanceMeshRecursive, // Don't use this yet
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
                illuminance: 2300000.0,
                shadows_enabled: true,
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

    // Sun Wall Refl
    commands.spawn((
        SpotLightBundle {
            transform: Transform::from_xyz(4.5, 4.0, 4.5)
                .looking_at(Vec3::new(-999.0, 0.0, 0.0), Vec3::Y),
            spot_light: SpotLight {
                range: 15.0,
                radius: 1.5,
                intensity: 250.0,
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
                    intensity: 1000.0,
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
                    intensity: 150.0,
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
    commands
        .spawn((
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
                    exposure: -2.0,
                    ..default()
                },
                ..default()
            },
            BloomSettings {
                intensity: 0.05,
                ..default()
            },
            EnvironmentMapLight {
                diffuse_map: asset_server.load("environment_maps/pisa_diffuse_rgb9e5_zstd.ktx2"),
                specular_map: asset_server.load("environment_maps/pisa_specular_rgb9e5_zstd.ktx2"),
            },
            CameraController::default().print_controls(),
        ))
        .insert(TemporalAntiAliasBundle::default())
        .insert(ScreenSpaceAmbientOcclusionBundle::default());
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
