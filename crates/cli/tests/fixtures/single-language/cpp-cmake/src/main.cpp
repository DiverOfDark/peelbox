#include <iostream>
#include <thread>
#include <chrono>

int main() {
    std::cout << "Hello from C++ E2E test server!" << std::endl;
    std::cout << "Listening on port 8080 (simulated)" << std::endl;
    
    while (true) {
        std::this_thread::sleep_for(std::chrono::seconds(1));
    }
    
    return 0;
}
