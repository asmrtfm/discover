// Import forms
import { Component } from "react";
import * as fs from "fs";
import path from "path";
import type { Config } from "./config";
import { type User, createUser } from "./users";

// Re-exports
export { helper } from "./helpers";
export * from "./utils";
export type { Options } from "./options";

// Namespace/module
namespace MyNamespace {
  export function inner(): void {}
}

module MyModule {
  export const val = 1;
}

// Interface
interface Greeter {
  greet(name: string): string;
}

// Type alias
type Result<T> = { ok: true; value: T } | { ok: false; error: Error };

// Enum
enum Direction {
  Up = "UP",
  Down = "DOWN",
  Left = "LEFT",
  Right = "RIGHT",
}

// Class
class Animal {
  name: string;
  static count: number = 0;

  constructor(name: string) {
    this.name = name;
    Animal.count++;
  }

  speak(): string {
    return `${this.name} makes a noise.`;
  }

  static getCount(): number {
    return Animal.count;
  }
}

// Abstract class
abstract class Shape {
  abstract area(): number;
  perimeter(): number {
    return 0;
  }
}

// Top-level functions
function greet(name: string): string {
  return `Hello, ${name}!`;
}

async function fetchData(url: string): Promise<Response> {
  return fetch(url);
}

// Arrow function assigned to const
const add = (a: number, b: number): number => a + b;

// Top-level const/let
const MAX_RETRIES = 3;
let currentRetry = 0;

// Decorated class
function sealed(constructor: Function) {}

@sealed
class BugReport {
  title: string;
  constructor(t: string) {
    this.title = t;
  }
}

// Generic function
function identity<T>(arg: T): T {
  return arg;
}

// Export default
export default class App {
  run(): void {}
}
