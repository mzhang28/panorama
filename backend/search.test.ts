import { describe, it, expect } from "bun:test";
import { parse } from "../generated/searchQuery";

describe("query parser", () => {
  it("should parse", () => {
    expect(parse("hello world")).toEqual(["hello", "world"]);
  });
});
