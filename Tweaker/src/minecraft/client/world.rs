use super::game::Game;
use super::*;
use jni_simple::{jtype, jtypes};

pub type Section = Box<[u16; 4096]>;

pub struct World<'a> {
    client: &'a Client,
    cache: &'a mut cache::Cache,
    world: jobject,
    pub position: [f64; 3],
}

impl Game {
    pub fn with_world<T>(&mut self, f: impl FnOnce(Option<&mut World<'_>>) -> Result<T>) -> Result<T> {
        self.scope(|game| unsafe {
            let c = &game.client;
            let field = game.cache.field(
                c,
                c.instance,
                "world",
                &["field_71441_e", "theWorld", "f"],
                &[
                    "Lnet/minecraft/client/multiplayer/WorldClient;",
                    "Lbdb;",
                    "Lbjf;",
                ],
            )?;
            let world = c.env.GetObjectField(c.instance, field);
            let player = game.cache.player(c)?;
            if world.is_null() || player.is_null() {
                return f(None);
            }
            let mut position = [0.0; 3];
            for (i, key, names) in [
                (0, "pos_x", ["field_70165_t", "posX", "s"]),
                (1, "pos_y", ["field_70163_u", "posY", "t"]),
                (2, "pos_z", ["field_70161_v", "posZ", "u"]),
            ] {
                let field = game.cache.field(c, player, key, &names, &["D"])?;
                position[i] = c.env.GetDoubleField(player, field);
            }
            if !position.iter().all(|v| v.is_finite()) {
                return f(None);
            }
            f(Some(&mut World {
                client: c,
                cache: &mut game.cache,
                world,
                position,
            }))
        })
    }
}

impl World<'_> {
    pub fn is(&self, global: jobject) -> bool {
        !global.is_null() && unsafe { self.client.env.IsSameObject(self.world, global) }
    }
    pub fn global(&self) -> jobject {
        unsafe { self.client.env.NewGlobalRef(self.world) }
    }

    pub fn section(&mut self, chunk_x: i32, section_y: i32, chunk_z: i32) -> Result<Option<Section>> {
        if !(0..16).contains(&section_y) {
            return Ok(None);
        }
        unsafe {
            let c = self.client;
            let method = self.cache.object_method(
                c,
                self.world,
                "plates_chunk",
                &["func_72964_e", "getChunkFromChunkCoords", "e", "a"],
                &[
                    "(II)Lnet/minecraft/world/chunk/Chunk;",
                    "(II)Lapx;",
                    "(II)Lamy;",
                ],
            )?;
            let chunk = c.env.CallObjectMethod2(self.world, method, chunk_x, chunk_z);
            exception::check(&c.env, "Bed Plates chunk read")?;
            if chunk.is_null() {
                return Ok(None);
            }
            let field = self.cache.field(
                c,
                chunk,
                "plates_storage",
                &["field_76652_q", "storageArrays", "u", "d"],
                &[
                    "[Lnet/minecraft/world/chunk/storage/ExtendedBlockStorage;",
                    "[Lapz;",
                    "[Lamz;",
                ],
            )?;
            let storage = c.env.GetObjectField(chunk, field);
            c.env.DeleteLocalRef(chunk);
            if storage.is_null() {
                return Ok(None);
            }
            let section = if section_y < c.env.GetArrayLength(storage.cast()) {
                c.env.GetObjectArrayElement(storage.cast(), section_y)
            } else {
                std::ptr::null_mut()
            };
            c.env.DeleteLocalRef(storage);
            exception::check(&c.env, "Bed Plates section read")?;
            if section.is_null() {
                return Ok(None);
            }
            let method = self.cache.object_method(
                c,
                section,
                "plates_states",
                &["func_177487_g", "getData", "g"],
                &["()[C"],
            )?;
            let data = c.env.CallObjectMethod0(section, method);
            c.env.DeleteLocalRef(section);
            exception::check(&c.env, "Bed Plates section states")?;
            if data.is_null() {
                return Ok(None);
            }
            if c.env.GetArrayLength(data.cast()) < 4096 {
                c.env.DeleteLocalRef(data);
                return Err(Error::Jvm("Bed Plates section states unavailable".into()));
            }
            let mut states: Section = Box::new([0; 4096]);
            c.env.GetCharArrayRegion(data.cast(), 0, 4096, states.as_mut_ptr());
            c.env.DeleteLocalRef(data);
            exception::check(&c.env, "Bed Plates bulk state read")?;
            Ok(Some(states))
        }
    }

    pub fn sidebar(&mut self) -> Result<Option<(String, Vec<String>)>> {
        unsafe {
            let c = self.client;
            let method = self.cache.object_method(
                c,
                self.world,
                "scoreboard",
                &["func_96441_U", "getScoreboard", "Z"],
                &["()Lnet/minecraft/scoreboard/Scoreboard;", "()Lauo;"],
            )?;
            let scoreboard = c.env.CallObjectMethod0(self.world, method);
            exception::check(&c.env, "Bed Plates scoreboard")?;
            if scoreboard.is_null() {
                return Ok(None);
            }
            let method = self.cache.object_method(
                c,
                scoreboard,
                "sidebar_objective",
                &["func_96539_a", "getObjectiveInDisplaySlot", "a"],
                &["(I)Lnet/minecraft/scoreboard/ScoreObjective;", "(I)Lauk;"],
            )?;
            let objective = c.env.CallObjectMethod1(scoreboard, method, 1i32);
            exception::check(&c.env, "Bed Plates sidebar")?;
            if objective.is_null() {
                return Ok(None);
            }
            let method = self.cache.object_method(
                c,
                objective,
                "objective_name",
                &["func_96678_d", "getDisplayName", "d"],
                &["()Ljava/lang/String;"],
            )?;
            let title = c.env.CallObjectMethod0(objective, method);
            exception::check(&c.env, "Bed Plates sidebar title")?;
            let title = self.string(title);
            let method = self.cache.object_method(
                c,
                scoreboard,
                "sorted_scores",
                &["func_96534_i", "getSortedScores", "i"],
                &[
                    "(Lnet/minecraft/scoreboard/ScoreObjective;)Ljava/util/Collection;",
                    "(Lauk;)Ljava/util/Collection;",
                ],
            )?;
            let scores = c.env.CallObjectMethod1(scoreboard, method, objective);
            exception::check(&c.env, "Bed Plates sidebar scores")?;
            let mut lines = Vec::new();
            if scores.is_null() {
                return Ok(Some((title, lines)));
            }
            let collection = self.cache.class(c, "collection", &["java.util.Collection"])?;
            let to_array = self.cache.method(
                c,
                collection,
                "collection_to_array",
                &["toArray"],
                &["()[Ljava/lang/Object;"],
                false,
            )?;
            let array = c.env.CallObjectMethod0(scores, to_array);
            exception::check(&c.env, "Bed Plates sidebar scores")?;
            if array.is_null() {
                return Ok(Some((title, lines)));
            }
            let team_class = self.cache.class(
                c,
                "score_team",
                &["net.minecraft.scoreboard.ScorePlayerTeam", "aul"],
            )?;
            let format = self.cache.method(
                c,
                team_class,
                "format_player_name",
                &["func_96667_a", "formatPlayerName", "a"],
                &[
                    "(Lnet/minecraft/scoreboard/Team;Ljava/lang/String;)Ljava/lang/String;",
                    "(Lauq;Ljava/lang/String;)Ljava/lang/String;",
                ],
                true,
            )?;
            let count = c.env.GetArrayLength(array.cast());
            for i in (0.max(count - 15)..count).rev() {
                let score = c.env.GetObjectArrayElement(array.cast(), i);
                if score.is_null() {
                    continue;
                }
                let method = self.cache.object_method(
                    c,
                    score,
                    "score_player",
                    &["func_96653_e", "getPlayerName", "e"],
                    &["()Ljava/lang/String;"],
                )?;
                let name = c.env.CallObjectMethod0(score, method);
                exception::check(&c.env, "Bed Plates sidebar line")?;
                if !name.is_null() {
                    let method = self.cache.object_method(
                        c,
                        scoreboard,
                        "players_team",
                        &["func_96509_i", "getPlayersTeam", "h"],
                        &[
                            "(Ljava/lang/String;)Lnet/minecraft/scoreboard/ScorePlayerTeam;",
                            "(Ljava/lang/String;)Laul;",
                        ],
                    )?;
                    let team = c.env.CallObjectMethod1(scoreboard, method, name);
                    exception::check(&c.env, "Bed Plates sidebar team")?;
                    let line = c.env.CallStaticObjectMethodA(team_class, format, jtypes!(team, name).as_ptr());
                    exception::check(&c.env, "Bed Plates sidebar line")?;
                    lines.push(self.string(line));
                    c.env.DeleteLocalRef(team);
                    c.env.DeleteLocalRef(name);
                }
                c.env.DeleteLocalRef(score);
            }
            c.env.DeleteLocalRef(array);
            Ok(Some((title, lines)))
        }
    }

    unsafe fn string(&self, text: jobject) -> String {
        if text.is_null() {
            return String::new();
        }
        let env = &self.client.env;
        let raw = unsafe { env.GetStringUTFChars_as_string(text.cast()) }.unwrap_or_default();
        unsafe { env.DeleteLocalRef(text) };
        plain(&raw)
    }
}

fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            chars.next();
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}
