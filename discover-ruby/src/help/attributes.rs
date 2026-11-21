use discover_core::help::{Page, attributes as core};

/// Ruby's `attributes`: what an ActiveRecord model and any other class carry, and how each is typed.
pub fn attributes() -> Page {
  Page {
    description: &[
      "List what instances of CLASS, declared in FILE, carry, one name per line.",
      "For an ActiveRecord model (a class whose superclass chain reaches ApplicationRecord or ActiveRecord::Base), the columns of its table in db/schema.rb (the table of its topmost concrete class, named by self.table_name or else the class name underscored and pluralized), its attribute declarations and its belongs_to/has_one/has_many/has_and_belongs_to_many associations. For any other class, its attribute declarations, associations and attr_* names.",
      "Columns and attribute declarations are typed with the Ruby class their values read as (Integer, String, DateTime, Array<String> for an array column), associations with their target class (Collection<X> for has_many and has_and_belongs_to_many), and names without a known type are untyped.",
    ],
    options: &[("-v, --verbose", "Print a JSON array of { NAME: TYPE } objects, one per line, instead; for an ActiveRecord model, also list its attr_accessor, attr_reader and attr_writer names.")],
    ..core::attributes()
  }
}
