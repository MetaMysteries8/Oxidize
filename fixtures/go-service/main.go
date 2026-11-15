// Detection fixture only: Go migration is not implemented yet.
package main

import (
	"fmt"
	"net/http"
)

func main() {
	http.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) { fmt.Fprint(w, "ok") })
	if err := http.ListenAndServe("127.0.0.1:8080", nil); err != nil {
		panic(err)
	}
}
