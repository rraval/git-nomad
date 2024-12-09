(function() {
    var type_impls = Object.fromEntries([["rustix",[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-AsRawFd-for-i32\" class=\"impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.48.0\">1.48.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#147\">source</a></span><a href=\"#impl-AsRawFd-for-i32\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"rustix/fd/trait.AsRawFd.html\" title=\"trait rustix::fd::AsRawFd\">AsRawFd</a> for <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.as_raw_fd\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#149\">source</a><a href=\"#method.as_raw_fd\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"rustix/fd/trait.AsRawFd.html#tymethod.as_raw_fd\" class=\"fn\">as_raw_fd</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h4></section></summary><div class='docblock'>Extracts the raw file descriptor. <a href=\"rustix/fd/trait.AsRawFd.html#tymethod.as_raw_fd\">Read more</a></div></details></div></details>","AsRawFd","rustix::fd::RawFd"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-FromRawFd-for-i32\" class=\"impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.48.0\">1.48.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#161\">source</a></span><a href=\"#impl-FromRawFd-for-i32\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"rustix/fd/trait.FromRawFd.html\" title=\"trait rustix::fd::FromRawFd\">FromRawFd</a> for <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.from_raw_fd\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#163\">source</a><a href=\"#method.from_raw_fd\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"rustix/fd/trait.FromRawFd.html#tymethod.from_raw_fd\" class=\"fn\">from_raw_fd</a>(fd: <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h4></section></summary><div class='docblock'>Constructs a new instance of <code>Self</code> from the given raw file\ndescriptor. <a href=\"rustix/fd/trait.FromRawFd.html#tymethod.from_raw_fd\">Read more</a></div></details></div></details>","FromRawFd","rustix::fd::RawFd"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-IntoRawFd-for-i32\" class=\"impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.48.0\">1.48.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#154\">source</a></span><a href=\"#impl-IntoRawFd-for-i32\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"rustix/fd/trait.IntoRawFd.html\" title=\"trait rustix::fd::IntoRawFd\">IntoRawFd</a> for <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.into_raw_fd\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"https://doc.rust-lang.org/1.83.0/src/std/os/fd/raw.rs.html#156\">source</a><a href=\"#method.into_raw_fd\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"rustix/fd/trait.IntoRawFd.html#tymethod.into_raw_fd\" class=\"fn\">into_raw_fd</a>(self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.83.0/std/primitive.i32.html\">i32</a></h4></section></summary><div class='docblock'>Consumes this object, returning the raw underlying file descriptor. <a href=\"rustix/fd/trait.IntoRawFd.html#tymethod.into_raw_fd\">Read more</a></div></details></div></details>","IntoRawFd","rustix::fd::RawFd"]]]]);
    if (window.register_type_impls) {
        window.register_type_impls(type_impls);
    } else {
        window.pending_type_impls = type_impls;
    }
})()
//{"start":55,"fragment_lengths":[4425]}