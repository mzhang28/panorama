import Koa from "koa";
import koaBodyparser from "koa-bodyparser";
import scheduler from "node-schedule";

const app = new Koa();

app.use(koaBodyparser());
app.use(async (ctx) => {
  ctx.body = "Hello World!";
});

const port = process.env.PORT || 3000;

app.listen(port, () => {
  console.log(`Server listening at http://localhost:${port}`);
});
