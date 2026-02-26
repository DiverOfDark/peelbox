CC = gcc
CFLAGS = -O2 -Wall -std=c11
TARGET = server

all: $(TARGET)

$(TARGET): src/main.c
	$(CC) $(CFLAGS) -o $(TARGET) src/main.c

clean:
	rm -f $(TARGET)
