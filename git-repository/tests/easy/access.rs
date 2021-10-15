use crate::repo;

#[test]
fn easy_arc_is_send() {
    fn thread_boundary<T: Send + 'static>(_: T) {}
    thread_boundary(repo("make_references_repo.sh").unwrap().into_easy_arc());
    thread_boundary(repo("make_references_repo.sh").unwrap().into_easy_arc_exclusive());
}
