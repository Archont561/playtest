// Package dirty shows the shape PCC-S01 rejects in Go.
package dirty

import "fmt"

// Start reports a service start through debug printing.
func Start(port int) {
	fmt.Println("starting") // expect: pcc-s01-print-go
	fmt.Printf("port %d\n", port) // expect: pcc-s01-print-go
}
