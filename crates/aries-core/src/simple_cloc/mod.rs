use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::ops::{Add, AddAssign};
use std::path::Path;

use futures::stream::{self, StreamExt};
use git2::Repository;
use lazy_static::lazy_static;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::fs::walk_dir;

pub async fn calc(root: impl AsRef<Path>) -> io::Result<File> {
    let root = root.as_ref();

    let _ = Repository::discover(root)
        .map_err(|err| io::Error::new(io::ErrorKind::Unsupported, err))?;

    let entries = walk_dir(root, true, false)?;
    let file_paths = entries.into_iter().filter(|e| e.is_file()).collect::<Vec<_>>();

    let file_metas = file_paths
        .into_iter()
        .filter_map(|file_path| {
            let info =
                file_path.extension().and_then(|extension| MANAGER.get_by_extension(extension))?;
            Some((file_path, info))
        })
        .collect::<Vec<_>>();

    let summary = stream::iter(file_metas)
        .filter_map(|(file_path, info)| async move { calculate_file(file_path, info).await.ok() })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .reduce(|prev, next| prev + next)
        .unwrap_or_default();

    Ok(summary)
}

async fn calculate_file(file_path: impl AsRef<Path>, info: &Rule) -> io::Result<File> {
    let file_path = file_path.as_ref();

    let file = tokio::fs::File::open(file_path).await?;
    let mut reader = BufReader::new(file);

    let mut first_line = String::new();
    reader.read_line(&mut first_line).await?;

    if first_line.contains("Code generated") {
        return Ok(File::new("Generated", 0, 0, 0, 0, 0));
    }

    let Rule { language, single, multi, .. } = info;

    let content = tokio::fs::read_to_string(&file_path).await?;
    let metadata = file_path.metadata()?;
    let bytes = metadata.len();
    let mut blank = 0;
    let mut comment = 0;
    let mut code = 0;
    let mut in_comment: Option<(&str, &str)> = None;

    'here: for line in content.lines() {
        let line = line.trim();

        // empty line
        if line.is_empty() {
            blank += 1;
            continue;
        }

        // match single line comments
        for single in single {
            if line.starts_with(single) {
                comment += 1;
                continue 'here;
            }
        }

        // match multi line comments
        for (start, end) in multi {
            if let Some(d) = in_comment {
                if d != (*start, *end) {
                    continue;
                }
            }

            // multi line comments maybe in one line
            let mut same_line = false;
            if line.starts_with(start) {
                in_comment = match in_comment {
                    Some(_) => {
                        comment += 1;
                        in_comment = None;
                        continue 'here;
                    },
                    None => {
                        same_line = true;
                        Some((start, end))
                    },
                }
            }

            // This line is in comments
            if in_comment.is_some() {
                comment += 1;
                if line.ends_with(end) {
                    if same_line {
                        if line.len() >= (start.len() + end.len()) {
                            in_comment = None;
                        }
                    } else {
                        in_comment = None;
                    }
                }
                continue 'here;
            }
        }

        code += 1;
    }

    Ok(File::new(language, 1, bytes, blank, comment, code))
}

#[derive(Debug, Copy, Clone)]
pub struct File {
    pub language: &'static str,
    pub files: usize,
    pub bytes: u64,
    pub blank: usize,
    pub comment: usize,
    pub code: usize,
}

impl File {
    pub fn new(
        language: &'static str,
        files: usize,
        bytes: u64,
        blank: usize,
        comment: usize,
        code: usize,
    ) -> Self {
        Self { language, files, bytes, blank, comment, code }
    }
}

impl Default for File {
    fn default() -> Self {
        Self::new("Empty", 0, 0, 0, 0, 0)
    }
}

impl Add for File {
    type Output = File;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            language: self.language,
            files: self.files + rhs.files,
            bytes: self.bytes + rhs.bytes,
            blank: self.blank + rhs.blank,
            comment: self.comment + rhs.comment,
            code: self.code + rhs.code,
        }
    }
}

impl AddAssign for File {
    fn add_assign(&mut self, rhs: Self) {
        self.files += rhs.files;
        self.bytes += rhs.bytes;
        self.blank += rhs.blank;
        self.comment += rhs.comment;
        self.code += rhs.code;
    }
}

#[derive(Debug)]
struct Rule {
    language: &'static str,
    #[allow(dead_code)]
    file_ext: Vec<&'static str>,
    single: Vec<&'static str>,
    multi: Vec<(&'static str, &'static str)>,
}

impl Rule {
    #[inline]
    fn new(
        language: &'static str,
        file_ext: Vec<&'static str>,
        single: Vec<&'static str>,
        multi: Vec<(&'static str, &'static str)>,
    ) -> Self {
        Self { language, file_ext, single, multi }
    }
}

struct Manager {
    languages: HashMap<&'static str, Rule>,
    ext_to_language: HashMap<&'static str, &'static str>,
}

impl Manager {
    #[inline]
    fn get_by_extension(&self, ext: &OsStr) -> Option<&Rule> {
        ext.to_str()
            .and_then(|ext| self.ext_to_language.get(ext))
            .and_then(|language| self.languages.get(language))
    }
}

lazy_static! {
    static ref MANAGER: Manager = {
        let mut languages = HashMap::<&'static str, Rule>::new();
        let mut ext_to_language = HashMap::new();

        macro_rules! language {
            ($language: expr, $ext: expr, $single: expr, $multi: expr) => {{
                languages.insert($language, Rule::new($language, $ext, $single, $multi));
                for e in $ext {
                    ext_to_language.insert(e, $language);
                }
            }};
            ($language: expr, $ext: expr, $single: expr) => {
                language!($language, $ext, $single, vec![])
            };
            ($language: expr, $ext: expr) => {
                language!($language, $ext, vec![], vec![])
            };
        }

        language!("ABAP", vec!["abap"], vec!["*", "\\\""]);
        language!("ABNF", vec!["abnf"], vec![";"]);
        language!("ActionScript", vec!["as"], vec!["//"], vec![("/*", "*/")]);
        language!("Ada", vec!["ada", "adb", "ads", "pad"], vec!["--"]);
        language!("Agda", vec!["agda"], vec!["--"], vec![("{-", "-}")]);
        language!("Alloy", vec!["als"], vec!["--", "//"], vec![("/*", "*/")]);
        language!("Arduino C++", vec!["ino"], vec!["//"], vec![("/*", "*/")]);
        language!("Assembly", vec!["asm"], vec![";"]);
        language!("GNU Style Assembly", vec!["s"], vec!["//"], vec![("/*", "*/")]);
        language!("ASP", vec!["asa", "asp"], vec!["'", "REM"]);
        language!(
            "ASP.NET",
            vec!["asax", "ascx", "asmx", "aspx", "master", "sitemap", "webinfo"],
            vec![],
            vec![("<!--", "-->"), ("<%--", "-->")]
        );
        language!("Autoconf", vec!["in"], vec!["#", "dnl"]);
        language!("Automake", vec!["am"], vec!["#"]);
        language!("Bash", vec!["bash"], vec!["#"]);
        language!("Batch", vec!["bat", "btm", "cmd"], vec!["REM", "::"]);
        language!("Cabal", vec!["cabal"], vec!["--"], vec![("{-", "-}")]);
        language!("C", vec!["c"], vec!["//"], vec![("/*", "*/")]);
        language!("Ceylon", vec!["ceylon"], vec!["//"], vec![("/*", "*/")]);
        language!("C Header", vec!["h"], vec!["//"], vec![("/*", "*/")]);
        language!("Clojure", vec!["clj"], vec![";"]);
        language!("ClojureScript", vec!["cljs"], vec![";"]);
        language!("ClojureC", vec!["cljc"], vec![";"]);
        language!("CMake", vec!["cmake"], vec!["#"]);
        language!("Cobol", vec!["cob", "cbl", "ccp", "cobol", "cpy"], vec!["*"]);
        language!("CoffeeScript", vec!["coffee", "cjsx"], vec!["#"], vec![("###", "###")]);
        language!("Coq", vec!["v"], vec![], vec![("(*", "*)")]);
        language!(
            "C++",
            vec!["cc", "cpp", "cxx", "c++", "pcc", "tpp"],
            vec!["//"],
            vec![("/*", "*/")]
        );
        language!(
            "C++ Header",
            vec!["hh", "hpp", "hxx", "inl", "ipp"],
            vec!["//"],
            vec![("/*", "*/")]
        );
        language!("Crystal", vec!["crystal"], vec!["#"]);
        language!("C#", vec!["cs", "csx"], vec!["//"], vec![("/*", "*/")]);
        language!("CSS", vec!["css"], vec!["//"], vec![("/*", "*/")]);
        language!("D", vec!["d"], vec!["//"], vec![("/*", "*/")]);
        language!("DAML", vec!["daml"], vec!["--"], vec![("{-", "-}")]);
        language!("dart", vec!["dart"], vec!["//"], vec![("/*", "*/")]);
        language!("Emacs Lisp", vec!["el"], vec![";"]);
        language!("Elixir", vec!["ex", "exs"], vec!["#"]);
        language!("Elm", vec!["elm"], vec!["--"], vec![("{-", "-}")]);
        language!("Erlang", vec!["erl", "hrl"], vec!["%"]);
        language!(
            "Fortran",
            vec!["F", "f90", "f95", "f03", "f08", "f15", "f77", "f", "for", "ftn", "fpp"],
            vec!["!"]
        );
        language!("FreeMarker", vec!["ftl", "ftlh", "ftlx"], vec![], vec![("<#--", "-->")]);
        language!("F#", vec!["fs", "fsi", "fsx", "fsscript"], vec!["//"], vec![("(*", "*)")]);
        language!("Go", vec!["go"], vec!["//"], vec![("/*", "*/"), ("/**", "*/")]);
        language!("Go HTML", vec!["gohtml"], vec![], vec![("<!--", "-->"), ("{{/*", "*/}}")]);
        language!("GraphQL", vec!["gql", "graphql"], vec!["#"]);
        language!("Groovy", vec!["groovy", "grt", "gtpl", "gvy"], vec!["//"], vec![("/*", "*/")]);
        language!("Gradle", vec!["gradle"], vec!["//"], vec![("/*", "*/"), ("/**", "*/")]);
        language!("Haskell", vec!["hs"], vec!["--"], vec![("{-", "-}")]);
        language!("Haxe", vec!["hx"], vec!["//"], vec![("/*", "*/")]);
        language!("Html", vec!["html", "xhtml", "hml"], vec![], vec![("<!--", "-->")]);
        language!("Idris", vec!["idr", "lidr"], vec!["--"], vec![("{-", "-}")]);
        language!("Ini", vec!["ini"], vec![";", "#"]);
        language!("Java", vec!["java"], vec!["//"], vec![("/*", "*/")]);
        language!("JavaScript", vec!["js", "mjs"], vec!["//"], vec![("/*", "*/")]);
        language!("JSON", vec!["json"]);
        language!("JSX", vec!["jsx"], vec!["//"], vec![("/*", "*/")]);
        language!("Julia", vec!["jl"], vec!["#"], vec![("#=", "=#")]);
        language!("Jupyter Notebooks", vec!["ipynb"]);
        language!("Kotlin", vec!["kt", "kts"], vec!["//"], vec![("/*", "*/")]);
        language!("Less", vec!["less"], vec!["//"], vec![("/*", "*/")]);
        language!("LLVM", vec!["ll"], vec![";"]);
        language!("Lua", vec!["lua"], vec!["--"], vec![("--[[", "]]")]);
        language!("Lucius", vec!["lucius"], vec!["//"], vec![("/*", "*/")]);
        language!("Markdown", vec!["md", "markdown"]);
        language!("Mint", vec!["mint"]);
        language!("Nim", vec!["nim"], vec!["#"]);
        language!("Nix", vec!["nix"], vec![], vec![("/*", "*/")]);
        language!("Objective-C", vec!["m"], vec!["//"], vec![("/*", "*/")]);
        language!("Objective-C++", vec!["mm"], vec!["//"], vec![("/*", "*/")]);
        language!("OCaml", vec!["ml", "mli", "re", "rei"], vec![], vec![("/*", "*/")]);
        language!("Org", vec!["org"], vec!["#"]);
        language!("Pascal", vec!["pas", "pp"], vec!["//"], vec![("{", "}"), ("(*", "*)")]);
        language!("Perl", vec!["pl", "pm"], vec!["#"], vec![("=pod", "=cut")]);
        language!("Pest", vec!["pest"], vec!["//"]);
        language!("Plain Text", vec!["text", "txt"]);
        language!(
            "Php",
            vec!["php4", "php5", "php", "phtml"],
            vec!["#", "//"],
            vec![("/*", "*/"), ("/**", "*/")]
        );
        language!("PostCSS", vec!["pcss", "sss"], vec!["//"], vec![("/*", "*/")]);
        language!("Prolog", vec!["p", "pro"], vec!["%"]);
        language!("Protocol Buffer", vec!["proto"], vec!["//"]);
        language!(
            "PowerShell",
            vec!["ps1", "psm1", "psd1", "ps1xml", "cdxml", "pssc", "psc1"],
            vec!["#"],
            vec![("<#", "#>")]
        );
        language!("PureScript", vec!["purs"], vec!["--"], vec![("{-", "-}")]);
        language!("Python", vec!["py"], vec!["#"], vec![("'''", "'''"), (r#"""#, r#"""#)]);
        language!("QCL", vec!["qcl"], vec!["//"], vec![("/*", "*/")]);
        language!("R", vec!["r"], vec!["#"]);
        language!("Racket", vec!["rkt"], vec![";"], vec![("#|", "|#")]);
        language!("Rakefile", vec!["rake"], vec!["#"], vec![("=begin", "=end")]);
        language!("Rakudo", vec!["pl6", "pm6"], vec!["#"], vec![("=begin", "=end")]);
        language!("Rust", vec!["rs"], vec!["//", "///", "///!"], vec![("/*", "*/")]);
        language!("Ruby", vec!["rb"], vec!["#"], vec![("=begin", "=end")]);
        language!("Ruby HTML", vec!["erb", "rhtml"], vec![], vec![("<!--", "-->")]);
        language!("ReStructuredText", vec!["rst"]);
        language!("Sass", vec!["sass", "scss"], vec!["//"], vec![("/*", "*/")]);
        language!("Scala", vec!["scala", "sc"], vec!["//"], vec![("/*", "*/")]);
        language!("Scheme", vec!["scm", "ss"], vec![";"], vec![("#|", "|#")]);
        language!("Shell", vec!["sh"], vec!["#"]);
        language!("Solidity", vec!["sol"], vec!["//"], vec![("/*", "*/")]);
        language!("SQL", vec!["sql"], vec!["#", "--"], vec![("/*", "*/")]);
        language!("Stylus", vec!["styl"], vec!["//"], vec![("/*", "*/")]);
        language!("SVG", vec!["svg"], vec![], vec![("<!--", "-->")]);
        language!("Swift", vec!["swift"], vec!["//"], vec![("/*", "*/")]);
        language!("TCL", vec!["tcl"], vec!["#"]);
        language!("Terraform", vec!["tf", "tfvars"], vec!["#", "//"], vec![("/*", "*/")]);
        language!("TeX", vec!["tex", "sty"], vec!["%"]);
        language!("Thrift", vec!["thrift"], vec!["#", "//"], vec![("/*", "*/")]);
        language!("Toml", vec!["toml"], vec!["#"]);
        language!("TSX", vec!["tsx"], vec!["//"], vec![("/*", "*/")]);
        language!("TypeScript", vec!["ts"], vec!["//"], vec![("/*", "*/")]);
        language!("VBScript", vec!["vbs"], vec!["'", "REM"]);
        language!("Visual Basic", vec!["vb"], vec!["'"]);
        language!("Visual Studio Solution", vec!["sln"]);
        language!(
            "Visual Studio Project",
            vec!["vcproj", "vcxproj"],
            vec![],
            vec![("<!--", "-->")]
        );
        language!("Vim script", vec!["vim"], vec!["\\\""], vec![("\\\"", "\\\""), ("'", "'")]);
        language!("Vue", vec!["vue"], vec!["//"], vec![("<!--", "-->"), ("/*", "*/")]);
        language!("WebAssembly", vec!["wat", "wast"], vec![";;"]);
        language!("XML", vec!["xml"], vec![], vec![("<!--", "-->"), ("<![CDATA[", "]]>")]);
        language!("Yaml", vec!["yml", "yaml"], vec!["#"]);
        language!("Zig", vec!["zig"], vec!["//"]);
        language!("Zsh", vec!["zsh"], vec!["#"]);

        Manager { languages, ext_to_language }
    };
}
