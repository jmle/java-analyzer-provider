package com.example.calls;

import com.example.service.UserService;
import com.example.model.User;
import java.util.ArrayList;
import java.util.List;

public class MethodCallExample {

    private UserService userService;

    public MethodCallExample() {
        this.userService = new UserService();
    }

    public void createUsers() {
        // Constructor call
        User user1 = new User("John", "Doe");
        User user2 = new User("Jane", "Smith");

        // Method calls
        userService.save(user1);
        userService.save(user2);

        List<User> users = new ArrayList<>();
        users.add(user1);
        users.add(user2);

        // Chained method calls
        String name = user1.getFirstName().toUpperCase().trim();

        // Static method call
        System.out.println(name);
    }

    public User findUser(String id) {
        return userService.findById(id);
    }
}
