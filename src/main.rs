use rand::Rng;
use std::cmp;
use tcod::colors::*;
use tcod::console::*;
use tcod::map::{FovAlgorithm, Map as FovMap};

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;
const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 45;
const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 50,
};
const COLOR_DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};
const COLOR_LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 50,
};
const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;
const MAX_ROOM_MONSTERS: i32 = 3;
const LIMIT_FPS: i32 = 20;
const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;
const PLAYER: usize = 0;

struct Tcod {
    root: Root,
    con: Offscreen,
    fov: FovMap,
}

/// This is a generic object: the player, a monster, an item, the stairs...
/// It's always represented by a character on screen.
#[derive(Debug)]
struct Object {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
}

impl Object {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        Object {
            x,
            y,
            char,
            name: name.into(),
            color,
            alive: false,
            blocks,
        }
    }

    /// set the color and then draw the character that represents
    /// this object at its position
    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }
}

/// Tile for map and it's properties
#[derive(Clone, Copy, Debug)]
struct Tile {
    blocked: bool,
    block_site: bool,
    explored: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_site: false,
            explored: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_site: true,
            explored: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;
        (center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        // If either of these conditions is false, then the rectangles cannot intersect.
        // If both of these conditons are true, then the rectangles can intersect
        // on the X axis.
        let intersects_on_x_axis = (self.x1 <= other.x2) && (self.x2 >= other.x1);

        // If either of these conditions is false, then the rectangles cannot intersect.
        // If both of these conditons are true, then the rectangles can intersect
        // on the Y axis.
        let intersects_on_y_axis = (self.y1 <= other.y2) && (self.y2 >= other.y1);

        // If both of these conditions are true, then the rectangles intersect.
        intersects_on_x_axis && intersects_on_y_axis
    }
}

/// NOTE:
/// There’s a ton of different ways to create the map.
/// One common alternative is one continuous Vec with MAP_HEIGHT * MAP_WIDTH items.
/// To access a tile on (x, y), you would do map[y * MAP_WIDTH + x].
/// The advantage is that you only do one array lookup instead of two and iterating
/// over every object in the map is faster because they’re all in the same region of memory.
type Map = Vec<Vec<Tile>>;

struct Game {
    map: Map,
}

fn main() {
    // Set up Tcod fields
    tcod::system::set_fps(LIMIT_FPS);
    let root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    let con = Offscreen::new(MAP_WIDTH, MAP_HEIGHT);
    let fov = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    let mut tcod = Tcod { root, con, fov };

    // Set up player, npc and vector of objects (players are objects)
    let mut player = Object::new(0, 0, '@', "player", WHITE, true);
    player.alive = true;
    let mut objects = vec![player];

    let mut game = Game {
        map: make_map(&mut objects),
    };

    // populate the FOV map, according to the generated map
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_site,
                !game.map[x as usize][y as usize].blocked,
            );
        }
    }

    // force FOV "recompute" first time through the game loop
    let previous_player_position = (-1, -1);

    // Game loop
    while !tcod.root.window_closed() {
        tcod.con.clear();

        let fov_recompute = previous_player_position != (objects[PLAYER].x, objects[PLAYER].y);
        render_all(&mut tcod, &mut game, &objects, fov_recompute);

        tcod.root.flush();

        let exit = handle_keys(&mut tcod, &game.map, &mut objects);
        if exit {
            break;
        }
    }
}

fn handle_keys(tcod: &mut Tcod, map: &Map, objects: &mut [Object]) -> bool {
    use tcod::input::Key;
    use tcod::input::KeyCode::*;

    let key = tcod.root.wait_for_keypress(true);
    match key {
        // movement keys
        Key { code: Up, .. } => move_by(PLAYER, 0, -1, map, objects),
        Key { code: Down, .. } => move_by(PLAYER, 0, 1, map, objects),
        Key { code: Left, .. } => move_by(PLAYER, -1, 0, map, objects),
        Key { code: Right, .. } => move_by(PLAYER, 1, 0, map, objects),

        // toggle fullscreen
        Key {
            code: Enter,
            alt: true,
            ..
        } => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
        }
        // exit game
        Key { code: Escape, .. } => return true,
        _ => {}
    }

    false
}

fn make_map(objects: &mut Vec<Object>) -> Map {
    // fill map with "blocked" tiles
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];

    let mut rooms = vec![];
    for _ in 0..MAX_ROOMS {
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);

        let is_valid_location = !rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if is_valid_location {
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects);

            let (new_x, new_y) = new_room.center();

            if rooms.is_empty() {
                // this is the first room, where the player starts
                objects[PLAYER].set_pos(new_x, new_y);
            } else {
                // center coordinates of previous room
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                // coinflip (50/50 bool)
                if rand::random() {
                    create_h_tunnel(prev_x, new_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, new_y, new_x, &mut map);
                } else {
                    create_v_tunnel(prev_y, new_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, new_x, new_y, &mut map);
                }
            }
        }

        // add the new room to the list of rooms
        rooms.push(new_room);
    }

    map
}

fn render_all(tcod: &mut Tcod, game: &mut Game, objects: &[Object], fov_recompute: bool) {
    // recompute FOV if needed (the player moved or something)
    if fov_recompute {
        let player = &objects[PLAYER];
        tcod.fov
            .compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO)
    }
    // draw all objects in the list
    for object in objects {
        if tcod.fov.is_in_fov(object.x, object.y) {
            object.draw(&mut tcod.con);
        }
    }

    // go through all tiles, and set their background color
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let is_visible = tcod.fov.is_in_fov(x, y);
            let is_wall = game.map[x as usize][y as usize].block_site;
            let color = match (is_visible, is_wall) {
                // outside of field of view:
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                // inside fov:
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let is_explored = &mut game.map[x as usize][y as usize].explored;
            if is_visible {
                *is_explored = true;
            }
            if *is_explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    blit(
        &tcod.con,
        (0, 0),
        (SCREEN_WIDTH, SCREEN_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );
}

fn create_room(room: Rect, map: &mut Map) {
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    // horizontal tunnel. `min()` and `max()` are used in case `x1 > x2`
    for x in cmp::min(x1, x2)..(cmp::max(x1, x2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..(cmp::max(y1, y2) + 1) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<Object>) {
    // choose random number of monsters
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);

    for _ in 0..num_monsters {
        // choose random spot for this monster
        // gen_range is inclusive for arg1 and exclusive for arg2,
        // so we need to add 1 to the first argument to not put a
        // monster in a wall.
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, map, objects) {
            let mut monster = if rand::random::<f32>() < 0.8 {
                // 80% chance of getting an orc
                Object::new(x, y, 'o', "orc", DESATURATED_GREEN, true)
            } else {
                Object::new(x, y, 'V', "Vampire", DARK_RED, true)
            };

            monster.alive = true;
            objects.push(monster);
        }
    }
}

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[Object]) -> bool {
    // first test the map tile
    if map[x as usize][y as usize].blocked {
        return true;
    }

    // now check for any blocking objects
    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

/// move by the given amount
fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [Object]) {
    let (x, y) = objects[id].pos();
    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}
