// @license http://www.gnu.org/licenses/agpl-3.0.html AGPL-3.0
document.addEventListener("click", async (event) => {
    const link = event.target.closest("a.deeper_replies");
    if (!link || event.ctrlKey || event.metaKey || event.shiftKey || event.button !== 0) return;
    event.preventDefault();
    if (link.dataset.loading) return;
    link.dataset.loading = "1";
    const label = link.textContent;
    link.textContent = "Loading…";

    try {
        const response = await fetch(link.href);
        if (!response.ok) throw new Error(response.status);
        const page = new DOMParser().parseFromString(await response.text(), "text/html");

        // The link opens either the parent comment's thread, whose replies we
        // want, or a page with just the missing batch of comments.
        const parent = link.closest(".comment");
        const fetchedParent = parent && page.getElementById(parent.id);
        const loaded = fetchedParent
            ? [...fetchedParent.querySelector(":scope > details > .replies").children]
            : [...page.querySelectorAll(".thread")].flatMap((thread) => [...thread.children]).filter((el) => !el.matches(".thread_nav"));
        const fresh = loaded.filter((el) => (el.id ? !document.getElementById(el.id) : el.getAttribute("href") !== link.getAttribute("href")));
        if (!fresh.length) throw new Error("nothing new");

        const thread = link.parentElement;
        if (thread.matches(".thread")) {
            // More top-level comments: each gets a thread of its own.
            thread.replaceWith(
                ...fresh.map((el) => {
                    const wrapper = document.createElement("div");
                    wrapper.className = "thread";
                    wrapper.append(el);
                    return wrapper;
                }),
            );
        } else {
            link.replaceWith(...fresh);
        }
    } catch {
        link.textContent = label;
        delete link.dataset.loading;
        window.location.href = link.href;
    }
});
// @license-end
