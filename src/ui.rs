use std::process::CommandArgs;

use bevy::{ecs::system::SystemId, prelude::*, window::PrimaryWindow};

use crate::Root;

#[derive(Component)]
#[require(ContextActions)]
pub struct ContextMenuRoot;

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, spawn_right_click_menu)
        .add_systems(Update, (update_right_click, context_buttons))
        .add_systems(Last, run_context_action)
        .add_event::<ContextEvent>();
}

fn spawn_right_click_menu(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Px(250.),
                height: Val::Px(100.),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            BorderRadius::all(Val::Px(10.)),
            Outline::new(Val::Px(5.), Val::Auto, Color::srgb(0.66, 0.33, 0.)),
            BackgroundColor(Color::srgb(0.66, 0.66, 0.66)),
            ContextMenuRoot,
        ))
        .with_children(|p| {
            p.spawn((
                Text("Contex: Tital".into()),
                BorderRadius::top(Val::Px(10.)),
                BackgroundColor(Color::srgb(0.5, 0.5, 0.5)),
            ));
        });
}

fn update_right_click(
    mut commands: Commands,
    mut clicks: EventReader<Pointer<Click>>,
    mut text: Query<&mut Text>,
    mut menu: Query<
        (
            Entity,
            &mut Node,
            &mut Visibility,
            &Children,
            &mut ContextActions,
        ),
        With<ContextMenuRoot>,
    >,
    actions: Query<&ContextActions, Without<ContextMenuRoot>>,
    names: Query<&Name, Or<(With<Root>, With<ContextActions>)>>,
    parent: Query<&Parent>,
    mut context_events: EventWriter<ContextEvent>,
) {
    for click in clicks.read() {
        let (root, mut menu, mut vis, children, mut current_actions) = menu.single_mut();

        for child in children.iter().skip(1) {
            commands.entity(*child).despawn_recursive();
        }
        if click.button != PointerButton::Secondary {
            continue;
        } else {
            *vis = Visibility::Visible;
            context_events.send(ContextEvent::Open);
        }
        menu.left = Val::Px(click.pointer_location.position.x);
        menu.top = Val::Px(click.pointer_location.position.y);
        let Ok(mut title) = text.get_mut(children[0]) else {
            error!("Menu Child 0 is not Text");
            continue;
        };
        title.0 = if let Ok(name) = names.get(click.target) {
            name.to_string()
        } else {
            format!("{:?}", click.target)
        };
        for entity in parent.iter_ancestors(click.target) {
            if let Ok(name) = names.get(entity) {
                title.0 = name.to_string();
                break;
            }
        }
        let mut context = actions.get(click.target).ok();
        if context.is_none() {
            for parent in parent.iter_ancestors(click.target) {
                if let Ok(parent) = actions.get(parent) {
                    context = Some(parent);
                    break;
                }
            }
        }
        if let Some(context) = context {
            *current_actions = context.clone();
            for (i, (option, _)) in context.options.iter().enumerate() {
                commands.entity(root).with_children(|p| {
                    p.spawn((Button, Text(option.clone()), ContextEvent::Run(i)));
                });
            }
        }
        commands.entity(root).with_children(|p| {
            p.spawn((Button, Text("Close".into()), ContextEvent::Close));
        });
    }
}

#[derive(Component, Default, Clone)]
pub struct ContextActions {
    pub on_open: Option<SystemId>,
    pub options: Vec<(String, SystemId)>,
    pub on_close: Option<SystemId>,
}

#[derive(Event, Debug, Clone, Copy)]
enum ContextEvent {
    Open,
    Close,
    Run(usize),
}

fn context_buttons(
    buttons: Query<(&Interaction, &ContextEvent), (With<Button>, Changed<Interaction>)>,
    mut root: Query<&mut Visibility, With<ContextMenuRoot>>,
    mut context_events: EventWriter<ContextEvent>,
) {
    for (interaction, event) in &buttons {
        if let Interaction::Pressed = interaction {
            context_events.send(*event);
            *root.single_mut() = Visibility::Hidden;
        }
    }
}

fn run_context_action(world: &mut World) {
    world.resource_scope(|world: &mut World, mut events: Mut<Events<ContextEvent>>| {
        for event in events.get_cursor().read(&events) {
            let mut run = None;
            match event {
                ContextEvent::Open => {
                    run = QueryState::<&ContextActions, With<ContextMenuRoot>>::new(world)
                        .single(world)
                        .on_open;
                }
                ContextEvent::Close => {
                    run = QueryState::<&ContextActions, With<ContextMenuRoot>>::new(world)
                        .single(world)
                        .on_close;
                }
                ContextEvent::Run(index) => {
                    run = QueryState::<&ContextActions, With<ContextMenuRoot>>::new(world)
                        .single(world)
                        .options
                        .get(*index)
                        .map(|(_, id)| *id);
                }
            }
            if let Some(run) = run {
                if let Err(e) = world.run_system(run) {
                    error!("{e}");
                };
            }
        }
        events.clear();
    })
}
