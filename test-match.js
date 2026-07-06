const subpath = "page/123";
const subpath2 = "/page/123";
console.log(subpath.match(/^page\/(.+)$/));
console.log(subpath2.match(/^page\/(.+)$/));
console.log(subpath2.match(/^\/?page\/(.+)$/));
