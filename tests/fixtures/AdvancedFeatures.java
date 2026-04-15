package com.example.advanced;

import java.util.List;
import java.util.ArrayList;
import org.springframework.stereotype.Service;
import org.springframework.beans.factory.annotation.Autowired;

@Service
@Deprecated(since = "1.0")
public class AdvancedFeatures extends BaseClass implements Runnable, Cloneable {

    @Autowired
    private UserService userService;

    private List<String> items;

    public AdvancedFeatures() {
        // Constructor call
        this.items = new ArrayList<>();
    }

    @Override
    public void run() {
        // Method calls
        String name = userService.getUserName();
        System.out.println(name);

        // Chained method calls
        items.add("test").toString().length();

        // Constructor call in expression
        User user = new User("John", 30);
        userService.save(user);
    }

    public void processData() {
        // Multiple method calls
        getData().stream()
            .filter(x -> x != null)
            .map(String::toUpperCase)
            .forEach(System.out::println);
    }

    private List<String> getData() {
        return items;
    }
}

class BaseClass {
    protected int value;
}

interface UserService {
    String getUserName();
    void save(User user);
}

class User {
    private String name;
    private int age;

    public User(String name, int age) {
        this.name = name;
        this.age = age;
    }
}
