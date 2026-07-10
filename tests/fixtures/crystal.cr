# Require forms
require "json"
require "http/client"
require "./config"
require "./models/*"
require "../utils"

# Module
module MyApp
  VERSION = "1.0.0"

  # Class
  class Server
    getter port : Int32
    getter host : String

    def initialize(@host : String, @port : Int32)
    end

    def start
      puts "Starting #{@host}:#{@port}"
    end

    def self.default
      new("localhost", 3000)
    end
  end

  # Struct
  struct Config
    property name : String
    property debug : Bool

    def initialize(@name, @debug = false)
    end
  end

  # Enum
  enum Status
    Active
    Inactive
    Pending
  end

  # Module method
  def self.run
    server = Server.default
    server.start
  end
end

# Top-level class
class AppError < Exception
  def initialize(@message : String)
  end
end

# Macro
macro define_method(name, content)
  def {{name}}
    {{content}}
  end
end

# Top-level method
def main
  MyApp.run
end
