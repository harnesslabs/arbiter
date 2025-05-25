use skeptic::markdown_files_of_directory;

extern crate skeptic;

fn main() {
  let markdown_files = markdown_files_of_directory("src/");
  skeptic::generate_doc_tests(&markdown_files);
}
