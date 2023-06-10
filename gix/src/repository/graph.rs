use gix_odb::Find;

impl crate::Repository {
    /// Create a graph data-structure capable of accelerating graph traversals and storing state of type `T` with each commit
    /// it encountered.
    ///
    /// Note that the commitgraph will be used if it is present and readable, but it won't be an error if it is corrupted. In that case,
    /// it will just not be used.
    ///
    /// ### Performance
    ///
    /// Note that the [Graph][gix_revision::Graph] can be sensitive to various object database settings that may affect the performance
    /// of the commit walk.
    pub fn revision_graph<T>(&self) -> gix_revision::Graph<'_, T> {
        gix_revision::Graph::new(
            |id, buf| {
                self.objects
                    .try_find(id, buf)
                    .map(|r| r.and_then(|d| d.try_into_commit_iter()))
            },
            gix_commitgraph::at(self.objects.store_ref().path().join("info")).ok(),
        )
    }

    /// Return a cache for commits and their graph structure, as managed by `git commit-graph`, for accelerating commit walks on
    /// a low level.
    ///
    /// Note that [`revision_graph()`][crate::Repository::revision_graph()] should be preferred for general purpose walks that don't
    /// rely on the actual commit cache to be present, while leveraging it if possible.
    pub fn commit_graph(&self) -> Result<gix_commitgraph::Graph, gix_commitgraph::init::Error> {
        gix_commitgraph::at(self.objects.store_ref().path().join("info"))
    }
}
