use eframe::egui::{self, ScrollArea};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::convert::TryFrom;

type SharedCache = Arc<Mutex<Vec<MealDetail>>>;
const API_BASE: &str = "https://www.themealdb.com/api/json/v1/1";

#[derive(Debug, Deserialize, Clone)]
struct MealShort {
    idMeal: String,
    strMeal: String,
}

#[derive(Debug, Deserialize, Clone)]
struct MealsList {
    meals: Option<Vec<MealShort>>,
}

#[derive(Debug, Deserialize, Clone)]
struct MealFull {
    idMeal: String,
    strMeal: String,
    strCategory: Option<String>,
    strArea: Option<String>,
    strInstructions: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
struct MealDetail {
    id: String,
    title: String,
    category: String,
    area: String,
    instructions: String,
    ingredients: Vec<String>,
    score: i32,
}

fn extract_ingredients(full: &MealFull) -> Vec<String> {
    let mut list = Vec::new();
    for i in 1..=20 {
        let key = format!("strIngredient{}", i);
        if let Some(val) = full.extra.get(&key) {
            if let Some(s) = val.as_str() {
                let ing = s.trim();
                if !ing.is_empty() {
                    list.push(ing.to_string());
                }
            }
        }
    }
    list
}

// Scoring: main ingredients higher priority
fn score_meal(
    detail: &MealDetail,
    main_ing: &[String],
    sub_ing: &[String],
    taste: &Option<String>,
) -> i32 {
    let mut score = 0;
    for ing in &detail.ingredients {
        for want in main_ing {
            if ing.to_lowercase().contains(&want.to_lowercase()) {
                score += 4;
            }
        }
        for want in sub_ing {
            if ing.to_lowercase().contains(&want.to_lowercase()) {
                score += 2;
            }
        }
    }
    if let Some(t) = taste {
        if detail.title.to_lowercase().contains(&t.to_lowercase()) {
            score += 3;
        }
    }
    score
}

fn fetch_candidates_by_ingredients(client: &Client, ingredients: &[String]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for ing in ingredients {
        let url = format!("{}/filter.php", API_BASE);
        if let Ok(resp) = client.get(&url).query(&[("i", ing)]).send() {
            if let Ok(list) = resp.json::<MealsList>() {
                if let Some(meals) = list.meals {
                    for m in meals {
                        ids.insert(m.idMeal);
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    ids
}

fn lookup_meal(client: &Client, id: &str) -> Option<MealFull> {
    let url = format!("{}/lookup.php", API_BASE);
    let res = client.get(&url).query(&[("i", id)]).send().ok()?;
    res.json::<HashMap<String, serde_json::Value>>()
        .ok()
        .and_then(|mut map| {
            map.remove("meals").and_then(|v| {
                v.as_array().and_then(|arr| {
                    arr.get(0)
                        .and_then(|obj| serde_json::from_value(obj.clone()).ok())
                })
            })
        })
}

fn pretty_print(meal: &MealDetail) {
    println!("\n====== {} ======", meal.title);
    println!("Category: {}", meal.category);
    println!("Area: {}", meal.area);
    println!("Ingredients: {:?}", meal.ingredients);
    println!("Instructions: {}", meal.instructions);
}

#[derive(Default)]
struct RecipeApp {
    taste: String,
    main_ingredients: String,
    sub_ingredients: String,
    cache: SharedCache,
    top_recipe_index: Option<usize>,
}

impl RecipeApp {
    fn fetch_recipes(&mut self) {
        let taste_opt = if self.taste.trim().is_empty() {
            None
        } else {
            Some(self.taste.clone())
        };

        let main_ing: Vec<String> = self
            .main_ingredients
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let sub_ing: Vec<String> = self
            .sub_ingredients
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut all_ing = main_ing.clone();
        all_ing.extend(sub_ing.clone());

        let cache_arc = Arc::new(Mutex::new(Vec::new()));
        self.cache = Arc::clone(&cache_arc);

        let main_clone = main_ing.clone();
        let sub_clone = sub_ing.clone();
        let taste_clone = taste_opt.clone();
        let all_clone = all_ing.clone();

        thread::spawn(move || {
            let client = Client::new();

            let ids_main = if !main_clone.is_empty() {
                fetch_candidates_by_ingredients(&client, &main_clone)
            } else {
                HashSet::new()
            };

            let ids_to_use: HashSet<String> = if !ids_main.is_empty() {
                ids_main
            } else {
                fetch_candidates_by_ingredients(&client, &all_clone)
            };

            for id in ids_to_use {
                if let Some(full) = lookup_meal(&client, &id) {
                    let meal = MealDetail {
                        id: full.idMeal.clone(),
                        title: full.strMeal.clone(),
                        category: full.strCategory.clone().unwrap_or_default(),
                        area: full.strArea.clone().unwrap_or_default(),
                        instructions: full.strInstructions.clone().unwrap_or_default(),
                        ingredients: extract_ingredients(&full),
                        score: 0,
                    };
                    let mut lock = cache_arc.lock().unwrap();
                    lock.push(meal);
                }
            }

            let mut lock = cache_arc.lock().unwrap();
            for m in lock.iter_mut() {
                m.score = score_meal(m, &main_clone, &sub_clone, &taste_clone);
            }
            lock.sort_by(|a, b| b.score.cmp(&a.score));
        });
    }
}

// Helper function for safe number parsing
fn parse_index(input: &str, max: usize) -> Option<usize> {
    let n = input.trim().parse::<i32>().ok()?;
    let idx = usize::try_from(n).ok()?;
    if idx == 0 || idx > max {
        None
    } else {
        Some(idx - 1)
    }
}

impl eframe::App for RecipeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Food Recipe Finder (GUI)");

            ui.horizontal(|ui| {
                ui.label("Taste:");
                ui.text_edit_singleline(&mut self.taste);
            });
            ui.horizontal(|ui| {
                ui.label("Main ingredients:");
                ui.text_edit_singleline(&mut self.main_ingredients);
            });
            ui.horizontal(|ui| {
                ui.label("Sub ingredients:");
                ui.text_edit_singleline(&mut self.sub_ingredients);
            });

            if ui.button("Fetch Recipes").clicked() {
                self.fetch_recipes();
            }

            let cache_lock = self.cache.lock().unwrap();
            if !cache_lock.is_empty() {
                ui.separator();
                ui.label("Top recipes:");
                for (i, meal) in cache_lock.iter().enumerate().take(10) {
                    if ui.button(format!("{}: {} (Score {})", i + 1, meal.title, meal.score)).clicked() {
                        self.top_recipe_index = Some(i);
                    }
                }
            }

            if let Some(index) = self.top_recipe_index {
                if let Some(meal) = cache_lock.get(index) {
                    ui.separator();
                    ui.label("Recipe Details:");
                    ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            ui.heading(&meal.title);
                            ui.label(format!("Category: {}", meal.category));
                            ui.label(format!("Area: {}", meal.area));
                            ui.separator();
                            ui.label("Ingredients:");
                            for ing in &meal.ingredients {
                                ui.label(format!("- {}", ing));
                            }
                            ui.separator();
                            ui.label("Instructions:");
                            ui.label(&meal.instructions);
                        });
                }
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Food Recipe Finder GUI",
        options,
        Box::new(|_cc| Ok(Box::new(RecipeApp::default()))),
    )
}
