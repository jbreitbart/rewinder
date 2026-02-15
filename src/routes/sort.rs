use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("desc") => SortDir::Desc,
            _ => SortDir::Asc,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }
}

pub fn apply_sort_dir(ordering: Ordering, sort_dir: SortDir) -> Ordering {
    match sort_dir {
        SortDir::Asc => ordering,
        SortDir::Desc => ordering.reverse(),
    }
}
