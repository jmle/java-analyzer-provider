package com.example.simple;

import java.util.List;
import java.util.ArrayList;

public class Simple {
    private int value;
    private String name;

    public Simple(int value, String name) {
        this.value = value;
        this.name = name;
    }

    public int getValue() {
        return value;
    }

    public void setValue(int value) {
        this.value = value;
    }

    public String getName() {
        return name;
    }

    public void setName(String name) {
        this.name = name;
    }

    public List<String> getItems() {
        List<String> items = new ArrayList<>();
        items.add(name);
        return items;
    }
}
