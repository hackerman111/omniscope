use crate::app::citation_graph::CitationEdge;
use crate::app::find_download::SearchResultItem;

/// Represents the result of an asynchronous background task.
#[derive(Debug)]
pub enum AsyncResultType {
    /// Result from fetching the citation graph.
    CitationGraphLoaded(Result<Vec<CitationEdge>, String>),
    
    /// Result from fetching search results in Find & Download.
    FindDownloadResultsLoaded {
        column: crate::app::find_download::SearchColumn,
        results: Result<Vec<SearchResultItem>, String>,
    },
    
    // Additional tasks can be added here
}

pub fn spawn_citation_fetch(
    tx: tokio::sync::mpsc::UnboundedSender<crate::event::AppEvent>,
    _book_id: uuid::Uuid,
) {
    tokio::spawn(async move {
        // MOCK: simulate network delay
        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

        let edges = vec![
            CitationEdge {
                title: "Fake Cited Reference 1".into(),
                year: Some(2023),
                id: None,
                id_type: None,
                in_library: true,
            },
            CitationEdge {
                title: "Fake Related Work".into(),
                year: Some(2021),
                id: None,
                id_type: None,
                in_library: false,
            },
        ];

        let _ = tx.send(crate::event::AppEvent::AsyncResult(
            AsyncResultType::CitationGraphLoaded(Ok(edges)),
        ));
    });
}

pub fn spawn_find_download(
    tx: tokio::sync::mpsc::UnboundedSender<crate::event::AppEvent>,
    column: crate::app::find_download::SearchColumn,
    _query: String,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        
        // MOCK data
        let results = vec![SearchResultItem {
            title: format!("Fake Result from {:?}", column),
            authors: "AI Author".into(),
            year: Some(2025),
            source: "MockAPI".into(),
            format_or_metrics: "PDF".into(),
            in_library: false,
            download_available: true,
        }];

        let _ = tx.send(crate::event::AppEvent::AsyncResult(
            AsyncResultType::FindDownloadResultsLoaded {
                column,
                results: Ok(results),
            },
        ));
    });
}
