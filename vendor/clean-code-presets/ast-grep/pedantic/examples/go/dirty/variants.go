package dirty

import "fmt"

// Report exercises each direct fmt printing variant.
func Report(port int) {
	fmt.Print("starting") // expect: pcc-s01-print-go
	fmt.Println("ready", port) // expect: pcc-s01-print-go
	fmt.Printf("port=%d\n", port) // expect: pcc-s01-print-go
}
