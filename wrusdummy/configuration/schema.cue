package configuration
[string]: #Route

#Route: {
	path: string & != ""
    method: string & != ""
    response: #Response
}

#Response: {
    status?: *200 | int
    content: string
    type: string
}
