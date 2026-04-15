package com.example.inheritance;

import com.example.base.BaseClass;
import com.example.interfaces.Runnable;
import com.example.interfaces.Serializable;

public class InheritanceExample extends BaseClass implements Runnable, Serializable {

    @Override
    public void run() {
        System.out.println("Running");
    }

    @Override
    public void serialize() {
        System.out.println("Serializing");
    }
}
