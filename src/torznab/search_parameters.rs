#[derive(Debug, Default)]
pub struct GenericSearchParameters {
    /// The string search query.
    pub query: Option<String>,
    /// Categories to search in
    pub categories: Vec<i32>,
    /// Extended attribute names that should be included in results.
    pub attributes: Vec<String>,
    /// Specifies that all extended attributes should be included in the results. Overrules attrs.
    pub extended: Option<bool>,
    /// Number of items to skip in the result.
    pub offset: Option<i32>,
    /// Number of results to return. Limited by the limits element in the Capabilities.
    pub limit: Option<i32>,
}

impl GenericSearchParameters {
    pub fn builder() -> GenericSearchParametersBuilder {
        GenericSearchParametersBuilder::default()
    }

    /// Convert the search parameters to a query string.
    /// This will be prefixed with "&"
    pub fn to_params(&self) -> String {
        let mut params = String::new();

        if let Some(query) = &self.query {
            let encoded = urlencoding::encode(query);
            params.push_str(&format!("&q={}", encoded));
        }

        if self.categories.len() > 0 {
            params.push_str(&format!("&cat={}", 
                self.categories.iter()
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                .join(",")));
        }

        if self.attributes.len() > 0 {
            params.push_str(&format!("&attrs={}", self.attributes.join(",")));
        }

        if let Some(extended) = &self.extended {
            // Convert the boolean to an integer.
            let i = if *extended { 1 } else { 0 };

            params.push_str(&format!("&extended={}", i));
        }

        if let Some(offset) = &self.offset {
            params.push_str(&format!("&offset={}", offset));
        }

        if let Some(limit) = &self.limit {
            params.push_str(&format!("&limit={}", limit));
        }

        params
    }
}

#[derive(Debug, Default)]
pub struct GenericSearchParametersBuilder {
    params: GenericSearchParameters,
}

impl GenericSearchParametersBuilder {
    pub fn query(mut self, query: String) -> GenericSearchParametersBuilder {
        self.params.query = Some(query);
        self
    }

    pub fn categories(mut self, categories: &[i32]) -> GenericSearchParametersBuilder {
        self.params.categories.extend_from_slice(categories);
        self
    }

    pub fn category(mut self, category: i32) -> GenericSearchParametersBuilder {
        self.params.categories.push(category);
        self
    }

    pub fn attributes(mut self, attributes: &[String]) -> GenericSearchParametersBuilder {
        self.params.attributes.extend_from_slice(attributes);
        self
    }

    pub fn attribute(mut self, attribute: String) -> GenericSearchParametersBuilder {
        self.params.attributes.push(attribute);
        self
    }

    pub fn extended(mut self, extended: bool) -> GenericSearchParametersBuilder {
        self.params.extended = Some(extended);
        self
    }

    pub fn offset(mut self, offset: i32) -> GenericSearchParametersBuilder {
        self.params.offset = Some(offset);
        self
    }

    pub fn limit(mut self, limit: i32) -> GenericSearchParametersBuilder {
        self.params.limit = Some(limit);
        self
    }

    pub fn build(self) -> GenericSearchParameters {
        self.params
    }
}

#[derive(Debug)]
pub struct TVSearchParameters {
    // idk what this is tbh
    pub rid: Option<u32>,
    /// Id of the show on TVDB.
    pub tvdb_id: Option<u32>,
    /// Id of the show on TVMaze.
    pub tvmaze_id: Option<u32>,
    /// Season number
    pub season: Option<u16>,
    /// Episode number
    pub episode: Option<u16>,
}

impl TVSearchParameters {
    pub fn to_params(&self) -> String {
        let mut params = String::new();

        if let Some(rid) = &self.rid {
            params.push_str(&format!("&rid={}", rid));
        }

        if let Some(tvdb_id) = &self.tvdb_id {
            params.push_str(&format!("&tvdbid={}", tvdb_id));
        }

        if let Some(tvmaze_id) = &self.tvmaze_id {
            params.push_str(&format!("&tvmazeid={}", tvmaze_id));
        }

        if let Some(season) = &self.season {
            params.push_str(&format!("&season={}", season));
        }

        if let Some(episode) = &self.episode {
            params.push_str(&format!("&ep={}", episode));
        }

        params
    }
}

pub struct TVSearchParametersBuilder {
    params: TVSearchParameters,
}

impl TVSearchParametersBuilder {
    pub fn new() -> TVSearchParametersBuilder {
        TVSearchParametersBuilder {
            params: TVSearchParameters {
                rid: None,
                tvdb_id: None,
                tvmaze_id: None,
                season: None,
                episode: None,
            },
        }
    }

    pub fn rid(mut self, rid: u32) -> TVSearchParametersBuilder {
        self.params.rid = Some(rid);
        self
    }

    pub fn tvdb_id(mut self, tvdb_id: u32) -> TVSearchParametersBuilder {
        self.params.tvdb_id = Some(tvdb_id);
        self
    }

    pub fn tvmaze_id(mut self, tvmaze_id: u32) -> TVSearchParametersBuilder {
        self.params.tvmaze_id = Some(tvmaze_id);
        self
    }

    pub fn season(mut self, season: u16) -> TVSearchParametersBuilder {
        self.params.season = Some(season);
        self
    }

    pub fn episode(mut self, episode: u16) -> TVSearchParametersBuilder {
        self.params.episode = Some(episode);
        self
    }

    pub fn build(self) -> TVSearchParameters {
        self.params
    }
}

#[derive(Debug)]
pub struct MovieSearchParameters {
    /// Id of the movie on IMDB.
    pub imdb_id: Option<u32>,
}

impl MovieSearchParameters {
    pub fn to_params(&self) -> String {
        let mut params = String::new();

        if let Some(imdb_id) = &self.imdb_id {
            params.push_str(&format!("&imdbid={}", imdb_id));
        }

        params
    }
}

pub struct MovieSearchParametersBuilder {
    params: MovieSearchParameters,
}

impl MovieSearchParametersBuilder {
    pub fn new() -> MovieSearchParametersBuilder {
        MovieSearchParametersBuilder {
            params: MovieSearchParameters {
                imdb_id: None,
            },
        }
    }

    pub fn imdb_id(mut self, imdb_id: u32) -> MovieSearchParametersBuilder {
        self.params.imdb_id = Some(imdb_id);
        self
    }

    pub fn build(self) -> MovieSearchParameters {
        self.params
    }
}