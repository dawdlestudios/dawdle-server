#[derive(Debug, serde::Deserialize, Default)]
pub struct FrontMatter {
    #[serde(default)]
    pub title: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub date: Option<String>,

    #[serde(default)]
    pub css: Option<ListOrSingle<String>>,

    #[serde(default)]
    pub theme: Option<String>,

    #[serde(default)]
    pub head: Option<ListOrSingle<String>>,

    #[serde(default)]
    pub layout: Option<String>,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(untagged)]
pub enum ListOrSingle<T> {
    List(Vec<T>),
    Single(T),
}

impl ListOrSingle<String> {
    pub fn as_list(&self) -> Vec<&str> {
        match self {
            ListOrSingle::List(list) => list.iter().map(|s| s.as_str()).collect(),
            ListOrSingle::Single(single) => vec![single.as_str()],
        }
    }
}

fn merge_property(
    a: Option<ListOrSingle<String>>,
    b: Option<ListOrSingle<String>>,
) -> Option<ListOrSingle<String>> {
    match (a, b) {
        (Some(ListOrSingle::List(a)), Some(ListOrSingle::List(mut b))) => {
            b.extend(a.iter().cloned());
            Some(ListOrSingle::List(b))
        }
        (Some(ListOrSingle::Single(a)), Some(ListOrSingle::List(mut b))) => {
            b.push(a.clone());
            Some(ListOrSingle::List(b))
        }
        (Some(ListOrSingle::List(mut a)), Some(ListOrSingle::Single(b))) => {
            a.push(b.clone());
            Some(ListOrSingle::List(a))
        }
        (Some(ListOrSingle::Single(a)), Some(ListOrSingle::Single(b))) => {
            Some(ListOrSingle::List(vec![a.clone(), b.clone()]))
        }
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

impl FrontMatter {
    pub fn from_md(input: &str) -> Self {
        let mut lines = input.lines();
        let mut front_matter = String::new();

        if let Some(line) = lines.next() {
            if line == "---" {
                for line in lines {
                    if line == "---" {
                        break;
                    }
                    front_matter.push_str(line);
                    front_matter.push('\n');
                }
            }
        }
        serde_yml::from_str::<FrontMatter>(&front_matter).unwrap_or_default()
    }

    pub fn merge(&mut self, other: &FrontMatter) {
        self.css = merge_property(self.css.clone(), other.css.clone());
        self.head = merge_property(self.head.clone(), other.head.clone());
        self.title = self.title.clone().or(other.title.clone());
        self.description = self.description.clone().or(other.description.clone());
        self.date = self.date.clone().or(other.date.clone());
        self.theme = self.theme.clone().or(other.theme.clone());
    }

    pub fn html_head(&self) -> String {
        let mut head = String::new();

        if let Some(theme) = &self.theme {
            if let Some(url) = super::themes::THEMES.iter().find(|(name, _)| name == theme) {
                head.push_str(&format!("<link rel=\"stylesheet\" href=\"{}\">\n", url.1));
            }
        }

        if let Some(css) = &self.css {
            for c in css.as_list() {
                head.push_str(&format!("<link rel=\"stylesheet\" href=\"{}\">\n", c));
            }
        }

        if let Some(heads) = &self.head {
            for h in heads.as_list() {
                head.push_str(h);
            }
        }

        if let Some(title) = &self.title {
            head.push_str(&format!("<title>{}</title>\n", title));
        }

        if let Some(description) = &self.description {
            head.push_str(&format!(
                "<meta name=\"description\" content=\"{}\">\n",
                description
            ));
        }

        if let Some(date) = &self.date {
            head.push_str(&format!("<meta name=\"date\" content=\"{}\">\n", date));
        }

        head
    }
}
