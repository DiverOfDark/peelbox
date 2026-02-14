CXX = g++
CXXFLAGS = -std=c++17 -O2 -Wall
TARGET = app

all: $(TARGET)

$(TARGET): main.cpp
	$(CXX) $(CXXFLAGS) -o $(TARGET) main.cpp -lpthread

clean:
	rm -f $(TARGET)
