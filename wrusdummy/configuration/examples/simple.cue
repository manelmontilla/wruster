import (
    "github.com/manelmontilla/wruster/wrusddummy:configuration"
)

all_gets: {
    path: "/",
    method: "GET"
    response: {
        status: 200
        content: "payload"
        type: "text/plain"
    }
}

all_posts: {
    path: "/",
    method: "POST"
    response: {
        status: 201
        content: "payload"
        type: "text/plain"
    }
}
