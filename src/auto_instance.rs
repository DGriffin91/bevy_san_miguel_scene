use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use bevy::ecs::component::Component;
use bevy::math::*;
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::primitives::Aabb;
use bevy::utils::HashMap;

pub struct AutoInstancePlugin;
impl Plugin for AutoInstancePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (apply_auto_instance_recursive, consolidate_mesh_instances),
        );
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

#[derive(Component)]
pub struct AutoInstanceMaterial;

#[derive(Component)]
pub struct AutoInstanceMaterialRecursive;

#[derive(Component)]
pub struct AutoInstanceMesh;

#[derive(Component)]
pub struct AutoInstanceMeshRecursive;

pub fn apply_auto_instance_recursive(
    mut commands: Commands,
    material_entities: Query<Entity, With<AutoInstanceMaterialRecursive>>,
    mesh_entities: Query<Entity, With<AutoInstanceMeshRecursive>>,
    children_query: Query<&Children>,
) {
    for entity in &material_entities {
        if let Ok(children) = children_query.get(entity) {
            all_children(children, &children_query, &mut |entity| {
                commands.entity(entity).insert(AutoInstanceMaterial);
            });
            commands
                .entity(entity)
                .remove::<AutoInstanceMaterialRecursive>();
        }
    }
    for entity in &mesh_entities {
        if let Ok(children) = children_query.get(entity) {
            all_children(children, &children_query, &mut |entity| {
                commands.entity(entity).insert(AutoInstanceMesh);
            });
            commands
                .entity(entity)
                .remove::<AutoInstanceMeshRecursive>();
        }
    }
}

pub fn consolidate_material_instances<M: Material + MaterialHash>(
    mut commands: Commands,
    materials: ResMut<Assets<M>>,
    entities: Query<(Entity, &Handle<M>), With<AutoInstanceMaterial>>,
    mut instances: Local<HashMap<u64, Handle<M>>>,
    mut count: Local<u32>,
) {
    let mut print = false;
    for (entity, mat_h) in &entities {
        if let Some(mat) = materials.get(mat_h) {
            print = true;
            let h = mat.generate_hash();
            if let Some(instance_h) = instances.get(&h) {
                commands.entity(entity).insert(instance_h.clone());
                *count += 1;
            } else {
                instances.insert(h, mat_h.clone());
            }
            commands.entity(entity).remove::<AutoInstanceMaterial>();
        }
    }
    if print {
        println!("Duplicate material instances found: {}", *count);
        println!("Total unique materials: {}", instances.len());
    }
}

// Implement the MaterialHash trait for any material
pub trait MaterialHash {
    fn generate_hash(&self) -> u64;
}

impl MaterialHash for StandardMaterial {
    fn generate_hash(&self) -> u64 {
        let state = &mut DefaultHasher::new();
        hash_color(&self.base_color, state);
        self.base_color_texture.hash(state);
        hash_color(&self.emissive, state);
        self.emissive_texture.hash(state);
        self.perceptual_roughness.to_bits().hash(state);
        self.metallic.to_bits().hash(state);
        self.metallic_roughness_texture.hash(state);
        self.reflectance.to_bits().hash(state);
        self.diffuse_transmission.to_bits().hash(state);
        self.specular_transmission.to_bits().hash(state);
        self.thickness.to_bits().hash(state);
        self.ior.to_bits().hash(state);
        self.attenuation_distance.to_bits().hash(state);
        hash_color(&self.attenuation_color, state);
        self.normal_map_texture.hash(state);
        self.flip_normal_map_y.hash(state);
        self.occlusion_texture.hash(state);
        self.double_sided.hash(state);
        self.cull_mode.hash(state);
        self.unlit.hash(state);
        self.fog_enabled.hash(state);
        match self.alpha_mode {
            AlphaMode::Opaque => 798573452.hash(state),
            AlphaMode::Mask(m) => m.to_bits().hash(state),
            AlphaMode::Blend => 1345634567.hash(state),
            AlphaMode::Premultiplied => 297897363.hash(state),
            AlphaMode::Add => 36345667.hash(state),
            AlphaMode::Multiply => 48967896.hash(state),
        }
        self.depth_bias.to_bits().hash(state);
        self.depth_map.hash(state);
        self.parallax_depth_scale.to_bits().hash(state);
        self.parallax_mapping_method
            .reflect_hash()
            .unwrap()
            .hash(state);
        self.max_parallax_layer_count.to_bits().hash(state);
        self.opaque_render_method
            .reflect_hash()
            .unwrap()
            .hash(state);
        self.deferred_lighting_pass_id.hash(state);
        state.finish()
    }
}

pub fn hash_color<H: Hasher>(color: &Color, state: &mut H) {
    color.r().to_bits().hash(state);
    color.g().to_bits().hash(state);
    color.b().to_bits().hash(state);
    color.a().to_bits().hash(state);
}

pub struct MeshData {
    handle: Handle<Mesh>,
    midpoint: Vec3,
    first_vert: Vec3,
    aabb: Aabb,
    avg_vert_dist: f32,
}

pub fn consolidate_mesh_instances(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    mut entities: Query<(Entity, &Handle<Mesh>, &mut Transform, &mut Aabb), With<AutoInstanceMesh>>,
    mut instances: Local<HashMap<u64, Vec<MeshData>>>,
    mut count: Local<u32>,
) {
    let mut print = false;
    for (entity, mesh_h, mut transform, mut aabb) in &mut entities {
        if let Some(mesh) = meshes.get(mesh_h) {
            print = true;
            let state = &mut DefaultHasher::new();
            /*
            Given two meshes that are essentially the same, but have all their vertices shifted over and rotated,
                this tries to identify a match and translate/rotate instances to their correct locations.
            TOOO The rotation isn't working
            Also probably need to slightly more robustly make sure the meshes are essentially the same.
            TODO this also doesn't take into account meshes existing translations
                (in san miguel all the trans/rot/scale are applied and all the meshes are at 0,0,0)
             */
            mesh.attributes().count().hash(state);
            let (first_vert, avg_vert_dist) = avg_distances_from_first_vert(mesh);
            for (id, attribute) in mesh.attributes() {
                id.hash(state);
                attribute.get_bytes().len().hash(state);
            }
            let midpoint = get_midpoint(mesh);
            let h = state.finish();
            let new_mesh_data = MeshData {
                handle: mesh_h.clone(),
                midpoint,
                first_vert,
                avg_vert_dist,
                aabb: *aabb,
            };
            if let Some(instance_datas) = instances.get_mut(&h) {
                let mut found = false;
                for instance_data in instance_datas.iter() {
                    if (instance_data.avg_vert_dist - avg_vert_dist).abs() < 0.001 {
                        found = true;
                        let _rot = calculate_rotation(
                            instance_data.midpoint,
                            midpoint,
                            instance_data.first_vert,
                            first_vert,
                        );
                        *transform = Transform::from_translation(midpoint - instance_data.midpoint);
                        // TODO rotation isn't right
                        //.with_rotation(rot);

                        *aabb = instance_data.aabb;

                        commands.entity(entity).insert(instance_data.handle.clone());
                        *count += 1;
                    }
                }
                if !found {
                    instance_datas.push(new_mesh_data)
                }
            } else {
                instances.insert(h, vec![new_mesh_data]);
            }
            commands.entity(entity).remove::<AutoInstanceMesh>();
        }
    }
    if print {
        println!("Duplicate mesh instances found: {}", *count);
        println!("Total unique meshes: {}", instances.len());
    }
}

fn get_midpoint(mesh: &Mesh) -> Vec3 {
    let mut mid_point = dvec3(0.0, 0.0, 0.0);
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(verts)) => {
            for v in verts {
                mid_point += DVec3::from([v[0] as f64, v[1] as f64, v[2] as f64]);
            }
            mid_point /= verts.len() as f64;
        }
        _ => (),
    }
    vec3(mid_point.x as f32, mid_point.y as f32, mid_point.z as f32)
}

fn avg_distances_from_first_vert(mesh: &Mesh) -> (Vec3, f32) {
    let mut first_vert = vec3(0.0, 0.0, 0.0);
    let mut avg: f64 = 0.0;
    let mut len = 0;
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(verts)) => {
            len = verts.len();
            first_vert = Vec3::from(verts[0]);
            for v in verts {
                avg += first_vert.distance(Vec3::from(*v)) as f64;
            }
        }
        _ => (),
    }
    (first_vert, (avg / len as f64) as f32)
}

fn calculate_rotation(midpoint1: Vec3, midpoint2: Vec3, vertex1: Vec3, vertex2: Vec3) -> Quat {
    // Direction from midpoint to the first vertex of each mesh
    let dir1 = (vertex1 - midpoint1).normalize();
    let dir2 = (vertex2 - midpoint2).normalize();

    Quat::from_rotation_arc(dir2, dir1)
}
