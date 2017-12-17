use cgmath::{InnerSpace, vec2};
use hate::{Context, Sprite, Time};
use hate::scene::Action;
use hate::scene::action;
use hate::geom::Point;
use hate::gui;
use core::{ObjId, PlayerId, State};
use core::event::{ActiveEvent, Event};
use core::map::PosHex;
use core::event;
use core::effect::{self, Effect, LastingEffect, TimedEffect};
use core::execute::Phase;
use core::ability::Ability;
use game_view::GameView;
use map;

pub fn message(view: &mut GameView, context: &mut Context, pos: PosHex, text: &str) -> Box<Action> {
    let visible = [0.0, 0.0, 0.0, 1.0];
    let invisible = [0.0, 0.0, 0.0, 0.0];
    let mut sprite = gui::text_sprite(context, text, 0.1);
    let point = map::hex_to_point(view.tile_size(), pos);
    let point = Point(point.0 + vec2(0.0, view.tile_size()));
    sprite.set_pos(point);
    sprite.set_color(invisible);
    let action_show_hide = Box::new(action::Sequence::new(vec![
        Box::new(action::Show::new(&view.layers().text, &sprite)),
        Box::new(action::ChangeColorTo::new(&sprite, visible, Time(0.3))),
        Box::new(action::Sleep::new(Time(1.0))),
        // TODO: read the time from Config:
        Box::new(action::ChangeColorTo::new(&sprite, invisible, Time(1.0))),
        Box::new(action::Hide::new(&view.layers().text, &sprite)),
    ]));
    let time = action_show_hide.duration();
    let delta = Point(vec2(0.0, 0.3));
    let action_move = Box::new(action::MoveBy::new(&sprite, delta, time));
    Box::new(action::Fork::new(Box::new(action::Sequence::new(vec![
        Box::new(action::Fork::new(action_move)),
        action_show_hide,
    ]))))
}

fn show_blood_spot(view: &mut GameView, context: &mut Context, at: PosHex) -> Box<Action> {
    let mut blood = Sprite::from_path(context, "blood.png", view.tile_size() * 2.0);
    blood.set_color([1.0, 1.0, 1.0, 0.0]);
    let mut point = map::hex_to_point(view.tile_size(), at);
    point.0.y -= view.tile_size() * 0.5;
    blood.set_pos(point);
    let color_final = [1.0, 1.0, 1.0, 0.3];
    Box::new(action::Sequence::new(vec![
        Box::new(action::Show::new(&view.layers().blood, &blood)),
        Box::new(action::ChangeColorTo::new(&blood, color_final, Time(0.3))),
    ]))
}

fn show_flare_scale(
    view: &mut GameView,
    context: &mut Context,
    at: PosHex,
    color: [f32; 4],
    scale: f32,
) -> Box<Action> {
    let visible = color;
    let mut invisible = visible;
    invisible[3] = 0.0;
    let size = view.tile_size() * 2.0 * scale;
    let mut flare = Sprite::from_path(context, "white_hex.png", size); // TODO: use special sprite
    let point = map::hex_to_point(view.tile_size(), at);
    flare.set_pos(point);
    flare.set_color(invisible);
    Box::new(action::Sequence::new(vec![
        Box::new(action::Show::new(&view.layers().flares, &flare)),
        Box::new(action::ChangeColorTo::new(&flare, visible, Time(0.1))),
        Box::new(action::ChangeColorTo::new(&flare, invisible, Time(0.3))),
        Box::new(action::Hide::new(&view.layers().flares, &flare)),
    ]))
}

fn show_flare(
    view: &mut GameView,
    context: &mut Context,
    at: PosHex,
    color: [f32; 4],
) -> Box<Action> {
    show_flare_scale(view, context, at, color, 1.0)
}

fn arc_move(view: &mut GameView, sprite: &Sprite, diff: Point) -> Box<Action> {
    let len = diff.0.magnitude();
    let base_height = view.tile_size() * 2.0;
    let height = base_height * (len / 1.0);
    let base_time = 0.5;
    let time = Time(base_time * (len / 1.0));
    let duration_0_25 = Time(time.0 * 0.25);
    let up_fast = Point(vec2(0.0, height * 0.75));
    let up_slow = Point(vec2(0.0, height * 0.25));
    let down_slow = Point(vec2(0.0, -height * 0.25));
    let down_fast = Point(vec2(0.0, -height * 0.75));
    let up_and_down = Box::new(action::Sequence::new(vec![
        Box::new(action::MoveBy::new(sprite, up_fast, duration_0_25)),
        Box::new(action::MoveBy::new(sprite, up_slow, duration_0_25)),
        Box::new(action::MoveBy::new(sprite, down_slow, duration_0_25)),
        Box::new(action::MoveBy::new(sprite, down_fast, duration_0_25)),
    ]));
    let main_move = Box::new(action::MoveBy::new(sprite, diff, time));
    Box::new(action::Sequence::new(
        vec![Box::new(action::Fork::new(main_move)), up_and_down],
    ))
}

fn remove_brief_unit_info(view: &mut GameView, id: ObjId) -> Box<Action> {
    let mut actions: Vec<Box<Action>> = Vec::new();
    let sprites = view.unit_info_get(id);
    for sprite in sprites {
        let mut color = sprite.color();
        color[3] = 0.0;
        actions.push(Box::new(
            action::Fork::new(Box::new(action::Sequence::new(vec![
                Box::new(action::ChangeColorTo::new(&sprite, color, Time(0.4))),
                Box::new(action::Hide::new(&view.layers().text, &sprite)),
            ]))),
        ));
    }
    Box::new(action::Sequence::new(actions))
}

fn generate_brief_obj_info(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    id: ObjId,
) -> Box<Action> {
    let mut actions: Vec<Box<Action>> = Vec::new();
    let agent = state.parts().agent.get(id);
    let obj_pos = state.parts().pos.get(id).0;
    let strength = state.parts().strength.get(id);
    let size = 0.2 * view.tile_size();
    let mut point = map::hex_to_point(view.tile_size(), obj_pos);
    point.0.x += view.tile_size() * 0.8;
    point.0.y += view.tile_size() * 0.6;
    let mut dots = Vec::new();
    let base_x = point.0.x;
    for &(color, n) in &[
        ([0.0, 0.4, 0.0, 1.0], strength.strength.0),
        ([1.0, 0.1, 1.0, 1.0], agent.jokers.0),
        ([1.0, 0.0, 0.0, 1.0], agent.attacks.0),
        ([0.0, 0.0, 1.0, 1.0], agent.moves.0),
    ] {
        for _ in 0..n {
            dots.push((color, point));
            point.0.x -= size;
        }
        point.0.x = base_x;
        point.0.y -= size;
    }
    let mut sprites = Vec::new();
    for &(color, point) in &dots {
        let mut sprite = Sprite::from_path(context, "white_hex.png", size);
        sprite.set_pos(point);
        sprite.set_color([color[0], color[1], color[2], 0.0]);
        let action = Box::new(action::Fork::new(Box::new(action::Sequence::new(vec![
            Box::new(action::Show::new(&view.layers().text, &sprite)),
            Box::new(action::ChangeColorTo::new(&sprite, color, Time(0.1))),
        ]))));
        sprites.push(sprite);
        actions.push(action);
    }
    view.unit_info_set(id, sprites);
    Box::new(action::Sequence::new(actions))
}

pub fn refresh_brief_unit_info(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    id: ObjId,
) -> Box<Action> {
    let mut actions = Vec::new();
    if view.unit_info_check(id) {
        actions.push(remove_brief_unit_info(view, id));
    }
    if state.parts().agent.get_opt(id).is_some() {
        actions.push(generate_brief_obj_info(state, view, context, id));
    }
    Box::new(action::Sequence::new(actions))
}

pub fn visualize(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &Event,
    phase: Phase,
) -> Box<Action> {
    match phase {
        Phase::Pre => visualize_pre(state, view, context, event),
        Phase::Post => visualize_post(state, view, context, event),
    }
}

fn visualize_pre(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &Event,
) -> Box<Action> {
    let mut actions = Vec::new();
    actions.push(visualize_event(state, view, context, &event.active_event));
    for (&target_id, effects) in &event.instant_effects {
        // TODO: fork here?
        for effect in effects {
            actions.push(Box::new(action::Fork::new(visualize_instant_effect(
                state,
                view,
                context,
                target_id,
                effect,
            ))));
        }
    }
    for (&target_id, effects) in &event.timed_effects {
        for effect in effects {
            actions.push(visualize_lasting_effect(
                state,
                view,
                context,
                target_id,
                effect,
            ));
        }
    }
    Box::new(action::Sequence::new(actions))
}

fn visualize_post(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &Event,
) -> Box<Action> {
    let mut actions = Vec::new();
    for &id in &event.actor_ids {
        actions.push(refresh_brief_unit_info(state, view, context, id));
    }
    for &id in event.instant_effects.keys() {
        actions.push(refresh_brief_unit_info(state, view, context, id));
    }
    for &id in event.timed_effects.keys() {
        actions.push(refresh_brief_unit_info(state, view, context, id));
    }
    Box::new(action::Sequence::new(actions))
}

fn visualize_event(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &ActiveEvent,
) -> Box<Action> {
    match *event {
        ActiveEvent::Create(ref ev) => visualize_event_create(state, view, context, ev),
        ActiveEvent::MoveTo(ref ev) => visualize_event_move_to(state, view, context, ev),
        ActiveEvent::Attack(ref ev) => visualize_event_attack(state, view, context, ev),
        ActiveEvent::EndTurn(ref ev) => visualize_event_end_turn(state, view, context, ev),
        ActiveEvent::BeginTurn(ref ev) => visualize_event_begin_turn(state, view, context, ev),
        ActiveEvent::UseAbility(ref ev) => visualize_event_use_ability(state, view, context, ev),
        ActiveEvent::EffectTick(ref ev) => visualize_event_effect_tick(state, view, context, ev),
        ActiveEvent::EffectEnd(ref ev) => visualize_event_effect_end(state, view, context, ev),
    }
}

fn visualize_event_create(
    _: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::Create,
) -> Box<Action> {
    let point = map::hex_to_point(view.tile_size(), event.pos);
    // TODO: Move to some .ron config:
    let sprite_name = match event.prototype.as_str() {
        "swordsman" => "swordsman.png",
        "spearman" => "spearman.png",
        "imp" => "imp.png",
        "boulder" => "boulder.png",
        "bomb" => "bomb.png",
        _ => unimplemented!(),
    };
    let size = view.tile_size() * 2.0;
    let mut sprite = Sprite::from_path(context, sprite_name, size);
    sprite.set_color([1.0, 1.0, 1.0, 0.0]);
    sprite.set_pos(point);
    view.add_object(event.id, &sprite);
    Box::new(action::Sequence::new(vec![
        Box::new(action::Show::new(&view.layers().units, &sprite)),
        Box::new(action::ChangeColorTo::new(
            &sprite,
            [1.0, 1.0, 1.0, 1.0],
            Time(0.25),
        )),
    ]))
}

fn visualize_event_move_to(
    _: &State,
    view: &mut GameView,
    _: &mut Context,
    event: &event::MoveTo,
) -> Box<Action> {
    let sprite = view.id_to_sprite(event.id).clone();
    let mut actions: Vec<Box<Action>> = Vec::new();
    for step in event.path.steps() {
        let from = map::hex_to_point(view.tile_size(), step.from);
        let to = map::hex_to_point(view.tile_size(), step.to);
        let diff = Point(to.0 - from.0);
        actions.push(Box::new(action::MoveBy::new(&sprite, diff, Time(0.3))));
    }
    Box::new(action::Sequence::new(actions))
}

fn visualize_event_attack(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::Attack,
) -> Box<Action> {
    let sprite = view.id_to_sprite(event.attacker_id).clone();
    let map_to = state.parts().pos.get(event.target_id).0;
    let to = map::hex_to_point(view.tile_size(), map_to);
    let map_from = state.parts().pos.get(event.attacker_id).0;
    let from = map::hex_to_point(view.tile_size(), map_from);
    let diff = Point((to.0 - from.0) / 2.0);
    let mut actions: Vec<Box<Action>> = Vec::new();
    actions.push(Box::new(action::Sleep::new(Time(0.1)))); // TODO: ??
    if event.mode == event::AttackMode::Reactive {
        actions.push(Box::new(action::Sleep::new(Time(0.3)))); // TODO: ??
        actions.push(message(view, context, map_from, "reaction"));
    }
    actions.push(Box::new(action::MoveBy::new(&sprite, diff, Time(0.15))));
    actions.push(Box::new(
        action::MoveBy::new(&sprite, Point(-diff.0), Time(0.15)),
    ));
    actions.push(Box::new(action::Sleep::new(Time(0.1)))); // TODO: ??
    Box::new(action::Sequence::new(actions))
}

fn visualize_event_end_turn(
    _: &State,
    _: &mut GameView,
    _: &mut Context,
    _: &event::EndTurn,
) -> Box<Action> {
    Box::new(action::Sleep::new(Time(0.2)))
}

fn visualize_event_begin_turn(
    _: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::BeginTurn,
) -> Box<Action> {
    let visible = [0.0, 0.0, 0.0, 1.0];
    let invisible = [0.0, 0.0, 0.0, 0.0];
    let text = match event.player_id {
        PlayerId(0) => "YOUR TURN",
        PlayerId(1) => "ENEMY TURN",
        _ => unreachable!(),
    };
    let mut sprite = gui::text_sprite(context, text, 0.2);
    sprite.set_pos(Point(vec2(0.0, 0.0)));
    sprite.set_color(invisible);
    Box::new(action::Sequence::new(vec![
        Box::new(action::Show::new(&view.layers().text, &sprite)),
        Box::new(action::ChangeColorTo::new(&sprite, visible, Time(0.2))),
        Box::new(action::Sleep::new(Time(1.5))),
        Box::new(action::ChangeColorTo::new(&sprite, invisible, Time(0.3))),
        Box::new(action::Hide::new(&view.layers().text, &sprite)),
    ]))
}

fn visualize_event_use_ability(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::UseAbility,
) -> Box<Action> {
    let pos = state.parts().pos.get(event.id).0;
    let text = event.ability.to_str();
    let action_main: Box<Action> = match event.ability {
        Ability::Jump => {
            // TODO: extract to a separate function
            let sprite = view.id_to_sprite(event.id).clone();
            let from = state.parts().pos.get(event.id).0;
            let from = map::hex_to_point(view.tile_size(), from);
            let to = map::hex_to_point(view.tile_size(), event.pos);
            let diff = Point(to.0 - from.0);
            arc_move(view, &sprite, diff)
        }
        Ability::Explode => show_flare_scale(view, context, pos, [1.0, 0.0, 0.0, 0.7], 2.0),
        _ => Box::new(action::Sleep::new(Time(0.0))),
    };
    Box::new(action::Sequence::new(vec![
        message(view, context, pos, &format!("<{}>", text)),
        action_main,
    ]))
}

fn visualize_event_effect_tick(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::EffectTick,
) -> Box<Action> {
    let pos = state.parts().pos.get(event.id).0;
    match event.effect {
        LastingEffect::Poison => show_flare(view, context, pos, [0.0, 0.8, 0.0, 0.7]),
        LastingEffect::Stun => show_flare(view, context, pos, [1.0, 1.0, 1.0, 0.7]),
    }
}

fn visualize_event_effect_end(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    event: &event::EffectEnd,
) -> Box<Action> {
    let pos = state.parts().pos.get(event.id).0;
    // TODO: this code is duplicated:
    let s = match event.effect {
        LastingEffect::Poison => "Poisoned",
        LastingEffect::Stun => "Stunned",
    };
    message(view, context, pos, &format!("[{}] ended", s))
}

pub fn visualize_lasting_effect(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
    timed_effect: &TimedEffect,
) -> Box<Action> {
    let pos = state.parts().pos.get(target_id).0;
    let action_flare = match timed_effect.effect {
        LastingEffect::Poison => show_flare(view, context, pos, [0.0, 0.8, 0.0, 0.7]),
        LastingEffect::Stun => show_flare(view, context, pos, [1.0, 1.0, 1.0, 0.7]),
    };
    // TODO: code duplication:
    let s = match timed_effect.effect {
        LastingEffect::Poison => "Poisoned",
        LastingEffect::Stun => "Stunned",
    };
    Box::new(action::Sequence::new(vec![
        action_flare,
        // Box::new(action::Sleep::new(Time(0.25))),
        message(view, context, pos, &format!("[{}]", s)),
        // Box::new(action::Sleep::new(Time(0.25))),
    ]))
}

pub fn visualize_instant_effect(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
    effect: &Effect,
) -> Box<Action> {
    let main_action = match *effect {
        Effect::Kill => visualize_effect_kill(state, view, context, target_id),
        Effect::Vanish => visualize_effect_vanish(state, view, context, target_id),
        Effect::Stun => visualize_effect_stun(state, view, context, target_id),
        Effect::Wound(ref e) => visualize_effect_wound(state, view, context, target_id, e),
        Effect::Knockback(ref e) => visualize_effect_knockback(state, view, context, target_id, e),
        Effect::FlyOff(ref e) => visualize_effect_fly_off(state, view, context, target_id, e),
        Effect::Thrown(ref e) => visualize_effect_thrown(state, view, context, target_id, e),
        Effect::Miss => visualize_effect_miss(state, view, context, target_id),
    };
    Box::new(action::Sequence::new(vec![
        // Box::new(action::Sleep::new(Time(0.25))),
        main_action,
        // Box::new(action::Sleep::new(Time(0.25))),
    ]))
}

fn visualize_effect_kill(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
) -> Box<Action> {
    // TODO: vanish + blood & text
    let pos = state.parts().pos.get(target_id).0;
    let sprite = view.id_to_sprite(target_id).clone();
    view.remove_object(target_id);
    let dark = [0.1, 0.1, 0.1, 1.0];
    let invisible = [0.1, 0.1, 0.1, 0.0];
    Box::new(action::Sequence::new(vec![
        message(view, context, pos, "killed"),
        Box::new(action::Sleep::new(Time(0.25))),
        show_blood_spot(view, context, pos),
        Box::new(action::ChangeColorTo::new(&sprite, dark, Time(0.2))),
        Box::new(action::ChangeColorTo::new(&sprite, invisible, Time(0.2))),
        Box::new(action::Hide::new(&view.layers().units, &sprite)),
    ]))
}

fn visualize_effect_vanish(
    _: &State,
    view: &mut GameView,
    _: &mut Context,
    target_id: ObjId,
) -> Box<Action> {
    let sprite = view.id_to_sprite(target_id).clone();
    view.remove_object(target_id);
    let dark = [0.1, 0.1, 0.1, 1.0];
    let invisible = [0.1, 0.1, 0.1, 0.0];
    Box::new(action::Sequence::new(vec![
        Box::new(action::Sleep::new(Time(0.25))),
        Box::new(action::ChangeColorTo::new(&sprite, dark, Time(0.2))),
        Box::new(action::ChangeColorTo::new(&sprite, invisible, Time(0.2))),
        Box::new(action::Hide::new(&view.layers().units, &sprite)),
    ]))
}

// TODO:
fn visualize_effect_stun(
    _state: &State,
    _view: &mut GameView,
    _context: &mut Context,
    _target_id: ObjId,
) -> Box<Action> {
    Box::new(action::Sleep::new(Time(1.0)))
}

fn visualize_effect_wound(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
    effect: &effect::Wound,
) -> Box<Action> {
    let damage = effect.damage;
    let pos = state.parts().pos.get(target_id).0;
    let sprite = view.id_to_sprite(target_id).clone();
    let color_normal = sprite.color();
    let color_dark = [0.1, 0.1, 0.1, 1.0];
    Box::new(action::Sequence::new(vec![
        message(view, context, pos, &format!("wounded - {}", damage.0)),
        Box::new(action::ChangeColorTo::new(&sprite, color_dark, Time(0.2))),
        Box::new(action::ChangeColorTo::new(&sprite, color_normal, Time(0.2))),
        show_blood_spot(view, context, pos),
    ]))
}

fn visualize_effect_knockback(
    _: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
    effect: &effect::Knockback,
) -> Box<Action> {
    // TODO: show some rotating dusty clouds
    let sprite = view.id_to_sprite(target_id).clone();
    let from = map::hex_to_point(view.tile_size(), effect.from);
    let to = map::hex_to_point(view.tile_size(), effect.to);
    let diff = Point(to.0 - from.0);
    Box::new(action::Sequence::new(vec![
        message(view, context, effect.to, "bump"),
        Box::new(action::MoveBy::new(&sprite, diff, Time(0.15))),
    ]))
}

fn visualize_effect_fly_off(
    _: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
    effect: &effect::FlyOff,
) -> Box<Action> {
    // TODO: add rotating dusty clouds
    let sprite = view.id_to_sprite(target_id).clone();
    let from = map::hex_to_point(view.tile_size(), effect.from);
    let to = map::hex_to_point(view.tile_size(), effect.to);
    let diff = Point(to.0 - from.0);
    let action_move = arc_move(view, &sprite, diff);
    Box::new(action::Sequence::new(vec![
        message(view, context, effect.to, "fly off"),
        action_move,
    ]))
}

fn visualize_effect_thrown(
    _: &State,
    view: &mut GameView,
    _: &mut Context,
    target_id: ObjId,
    effect: &effect::Thrown,
) -> Box<Action> {
    let sprite = view.id_to_sprite(target_id).clone();
    let from = map::hex_to_point(view.tile_size(), effect.from);
    let to = map::hex_to_point(view.tile_size(), effect.to);
    let diff = Point(to.0 - from.0);
    arc_move(view, &sprite, diff)
}

fn visualize_effect_miss(
    state: &State,
    view: &mut GameView,
    context: &mut Context,
    target_id: ObjId,
) -> Box<Action> {
    let pos = state.parts().pos.get(target_id).0;
    message(view, context, pos, "missed")
}
