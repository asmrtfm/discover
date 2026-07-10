// Import forms
import 'dart:io';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:my_app/models/user.dart';
import 'package:my_app/screens/home.dart';
import '../widgets/button.dart';
import './helpers.dart';

// Export
export 'package:my_app/base/nav.dart';
export '../widgets/overlay.dart';

// Part
part 'user.g.dart';

// Enum
enum UserRole {
  admin,
  editor,
  viewer,
}

// Mixin
mixin Serializable {
  Map<String, dynamic> toJson();

  String serialize() {
    return jsonEncode(toJson());
  }
}

// Class
class UserProfile extends StatefulWidget {
  final String name;
  final int age;

  const UserProfile({required this.name, required this.age});

  @override
  State<UserProfile> createState() => _UserProfileState();
}

class _UserProfileState extends State<UserProfile> with Serializable {
  @override
  Widget build(BuildContext context) {
    return Container();
  }

  @override
  Map<String, dynamic> toJson() => {};
}

// Extension
extension StringUtils on String {
  String capitalize() => '${this[0].toUpperCase()}${substring(1)}';
}

// Typedef
typedef JsonMap = Map<String, dynamic>;
typedef Callback = void Function(String);

// Top-level functions
void main() {
  runApp(const MyApp());
}

Future<UserProfile> fetchUser(String id) async {
  final response = await HttpClient().getUrl(Uri.parse('/api/users/$id'));
  return UserProfile(name: 'test', age: 0);
}

String formatName(String first, String last) {
  return '$first $last';
}
