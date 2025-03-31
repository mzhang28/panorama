import { z } from "zod";
import { parse } from "../generated/searchQuery";
import zodRouter from "koa-zod-router";

export const SearchExpr = z.object({});

export const router = zodRouter({
  koaRouter: {
    prefix: "/search",
  },
});

router.post(
  "/query",
  (ctx) => {
    const { query } = ctx.request.body;
    const tree = parse(query);
    console.log("tree", tree);
  },
  { body: z.object({ query: z.string() }) },
);
