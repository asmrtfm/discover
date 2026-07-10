require "json"
require_relative "./foo"
require_relative "../bar"

module MyModule
  class MyClass
    def my_method
      puts "hello"
    end

    def self.class_method
      42
    end
  end
end

class TopClass < Base
  include Enumerable
  attr_reader :name

  def initialize(name)
    @name = name
  end
end

def top_level_func
  42
end
